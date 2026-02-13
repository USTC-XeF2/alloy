//! HTTP server capability implementation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use alloy_core::{
    BoxedConnectionHandler, ConnectionInfo, HttpServerCapability, ListenerHandle, TransportError,
    TransportResult,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Bytes,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

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
    handler: BoxedConnectionHandler,
    /// Track known clients by their address (for connection persistence).
    known_clients: RwLock<HashMap<String, String>>, // addr -> bot_id
}

#[async_trait]
impl HttpServerCapability for HttpServerCapabilityImpl {
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: BoxedConnectionHandler,
    ) -> TransportResult<ListenerHandle> {
        let state = Arc::new(ServerState {
            handler,
            known_clients: RwLock::new(HashMap::new()),
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
    body: Bytes,
) -> impl IntoResponse {
    let addr_str = addr.to_string();

    // Check if we know this client
    let bot_id = {
        let known = state.known_clients.read().await;
        known.get(&addr_str).cloned()
    };

    let bot_id = match bot_id {
        Some(id) => id,
        None => {
            // New client, call on_connect
            let conn_info = ConnectionInfo::new("http").with_remote_addr(addr_str.clone());

            let new_bot_id = match state.handler.on_connect(conn_info).await {
                Ok(id) => id,
                Err(e) => {
                    error!(error = %e, remote_addr = %addr, "Failed to establish HTTP connection");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to establish connection",
                    )
                        .into_response();
                }
            };

            // Store the mapping
            {
                let mut known = state.known_clients.write().await;
                known.insert(addr_str, new_bot_id.clone());
            }

            info!(bot_id = %new_bot_id, remote_addr = %addr, "New HTTP client connected");
            new_bot_id
        }
    };

    // Process the message
    trace!(bot_id = %bot_id, len = body.len(), "Received HTTP POST");

    if let Some(event) = state.handler.on_message(&bot_id, &body).await {
        debug!(bot_id = %bot_id, event = %event.event_name(), "Parsed event from HTTP POST");
    }

    // Return 200 OK
    (StatusCode::OK, "ok")
}
