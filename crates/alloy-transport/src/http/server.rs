//! HTTP server capability implementation.
//!
//! HTTP server bots can receive events via POST and potentially send
//! responses via Server-Sent Events (SSE) or other mechanisms.
//! Each bot gets a ConnectionHandle that adapters can use to implement
//! custom push strategies.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    Router,
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use tokio::sync::{RwLock, mpsc, watch};
use tracing::{debug, error, info, trace, warn};

use alloy_core::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, HttpServerCapability, ListenerHandle,
    TransportResult,
};

/// HTTP server capability implementation.
pub struct HttpServerCapabilityImpl;

impl HttpServerCapabilityImpl {
    /// Creates a new HTTP server capability.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpServerCapabilityImpl {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state for the HTTP server.
struct ServerState {
    /// Connection handler from the adapter.
    handler: Arc<dyn ConnectionHandler>,
    /// Track known bots and their connection handles.
    /// The ConnectionHandle allows adapters to implement custom push strategies (SSE, etc.)
    known_bots: RwLock<HashMap<String, ConnectionHandle>>,
}

#[async_trait]
impl HttpServerCapability for HttpServerCapabilityImpl {
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ListenerHandle> {
        let state = Arc::new(ServerState {
            handler,
            known_bots: RwLock::new(HashMap::new()),
        });

        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        let router = Router::new()
            .route(&path, post(http_handler))
            .with_state(state.clone());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_addr = listener.local_addr()?;

        info!(addr = %actual_addr, path = %path, "HTTP server listening");

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        // Spawn the server
        let server_state = state.clone();
        tokio::spawn(async move {
            let server = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            );

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        error!(error = %e, "HTTP server error");
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("HTTP server shutting down");
                    // Disconnect all known bots
                    let known = server_state.known_bots.read().await;
                    for (bot_id, handle) in known.iter() {
                        handle.close();
                        server_state.handler.on_disconnect(bot_id).await;
                    }
                }
            }
        });

        let handle = ListenerHandle::new(format!("http-server-{}", actual_addr), shutdown_tx);

        Ok(handle)
    }
}

/// HTTP POST handler.
async fn http_handler(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Build connection info with headers as metadata
    let mut conn_info = ConnectionInfo::new("http").with_remote_addr(addr.to_string());
    for (name, value) in &headers {
        if let Ok(value_str) = value.to_str() {
            conn_info =
                conn_info.with_metadata(name.as_str().to_lowercase(), value_str.to_string());
        }
    }

    // Extract bot ID from request metadata
    let bot_id = match state.handler.get_bot_id(conn_info).await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, remote_addr = %addr, "Failed to extract bot ID from HTTP request");
            return (StatusCode::BAD_REQUEST, "Failed to extract bot ID").into_response();
        }
    };

    // Create bot if first time seeing this bot_id
    {
        let known = state.known_bots.read().await;
        if !known.contains_key(&bot_id) {
            drop(known);
            let mut known = state.known_bots.write().await;
            // Double-check after acquiring write lock
            if !known.contains_key(&bot_id) {
                // Create channel for potential outgoing messages (SSE, webhooks, etc.)
                let (message_tx, mut message_rx) = mpsc::channel::<Vec<u8>>(256);
                let (shutdown_tx, _shutdown_rx) = watch::channel(false);

                let connection_handle =
                    ConnectionHandle::new(bot_id.clone(), message_tx, shutdown_tx);

                // Spawn task to handle outgoing messages
                // Adapters can implement custom logic to consume these messages (e.g., SSE push)
                let bot_id_clone = bot_id.clone();
                tokio::spawn(async move {
                    while let Some(data) = message_rx.recv().await {
                        // Default behavior: log that message was sent but cannot be delivered
                        // Adapters can override this by implementing custom HTTP response mechanisms
                        warn!(
                            bot_id = %bot_id_clone,
                            len = data.len(),
                            "HTTP bot sent message, but no delivery mechanism configured (consider implementing SSE)"
                        );
                    }
                });

                state
                    .handler
                    .create_bot(&bot_id, connection_handle.clone())
                    .await;
                known.insert(bot_id.clone(), connection_handle);
                info!(bot_id = %bot_id, remote_addr = %addr, "HTTP bot created");
            }
        }
    }

    // Process the message
    trace!(bot_id = %bot_id, len = body.len(), "Received HTTP POST");

    if let Some(event) = state.handler.on_message(&bot_id, &body).await {
        debug!(bot_id = %bot_id, event = %event.event_name(), "Parsed event from HTTP POST");
    }

    // Return 200 OK
    (StatusCode::OK, "ok").into_response()
}
