//! WebSocket server capability implementation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use alloy_core::{
    BoxedConnectionHandler, ConnectionHandle, ConnectionInfo, ListenerHandle, WsServerCapability,
};
use async_trait::async_trait;
use axum::{
    Router,
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, trace, warn};

/// WebSocket server capability implementation.
pub struct WsServerCapabilityImpl;

impl WsServerCapabilityImpl {
    /// Creates a new WebSocket server capability.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WsServerCapabilityImpl {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state for the WebSocket server.
struct ServerState {
    /// Connection handler from the adapter.
    handler: BoxedConnectionHandler,
    /// Active connections (bot_id -> sender).
    connections: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>,
}

#[async_trait]
impl WsServerCapability for WsServerCapabilityImpl {
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: BoxedConnectionHandler,
    ) -> anyhow::Result<ListenerHandle> {
        let state = Arc::new(ServerState {
            handler,
            connections: RwLock::new(HashMap::new()),
        });

        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        let router = Router::new()
            .route(&path, get(ws_handler))
            .with_state(state.clone());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_addr = listener.local_addr()?;

        info!(addr = %actual_addr, path = %path, "WebSocket server listening");

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
                        error!(error = %e, "WebSocket server error");
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("WebSocket server shutting down");
                    // Close all connections
                    let connections = server_state.connections.read().await;
                    for (bot_id, _) in connections.iter() {
                        server_state.handler.on_disconnect(bot_id).await;
                    }
                }
            }
        });

        let handle = ListenerHandle::new(format!("ws-server-{}", actual_addr), shutdown_tx);

        Ok(handle)
    }

    fn default_addr(&self) -> &str {
        "0.0.0.0:8080"
    }

    fn default_path(&self) -> &str {
        "/ws"
    }
}

/// WebSocket upgrade handler.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    info!(remote_addr = %addr, "New WebSocket connection request");

    // Extract headers as metadata (convert all to lowercase for consistency)
    let mut metadata = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            // Store header with lowercase key for consistent lookup
            metadata.insert(name.as_str().to_lowercase(), value_str.to_string());
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, addr, state, metadata))
}

/// Handles an individual WebSocket connection.
async fn handle_socket(
    socket: WebSocket,
    addr: SocketAddr,
    state: Arc<ServerState>,
    headers: HashMap<String, String>,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Create connection info with headers as metadata
    let mut conn_info = ConnectionInfo::new("websocket").with_remote_addr(addr.to_string());
    for (key, value) in headers {
        conn_info = conn_info.with_metadata(key, value);
    }

    // Call on_connect to get bot ID
    let bot_id = state.handler.on_connect(conn_info).await;

    info!(bot_id = %bot_id, remote_addr = %addr, "WebSocket connection established");

    // Create channel for sending messages to this connection
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);

    // Create shutdown channel
    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);

    // Create ConnectionHandle for this connection
    let connection_handle = ConnectionHandle::new(bot_id.clone(), tx.clone(), shutdown_tx);

    // Call on_ready with the connection handle
    state.handler.on_ready(&bot_id, connection_handle).await;

    // Register the connection
    {
        let mut connections = state.connections.write().await;
        connections.insert(bot_id.clone(), tx.clone());
    }

    // Spawn task to forward messages to the WebSocket
    let bot_id_send = bot_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_tx
                .send(Message::Text(
                    String::from_utf8_lossy(&data).to_string().into(),
                ))
                .await
                .is_err()
            {
                warn!(bot_id = %bot_id_send, "Failed to send message, connection closed");
                break;
            }
        }
    });

    // Receive messages from the WebSocket
    let handler = state.handler.clone();
    let bot_id_recv = bot_id.clone();
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(Message::Text(text)) => {
                trace!(bot_id = %bot_id_recv, len = text.len(), "Received text message");
                if let Some(event) = handler.on_message(&bot_id_recv, text.as_bytes()).await {
                    debug!(bot_id = %bot_id_recv, event = %event.event_name(), "Parsed event from message");
                    // Event is returned to the adapter's dispatcher via the handler
                }
            }
            Ok(Message::Binary(data)) => {
                trace!(bot_id = %bot_id_recv, len = data.len(), "Received binary message");
                if let Some(event) = handler.on_message(&bot_id_recv, &data).await {
                    debug!(bot_id = %bot_id_recv, event = %event.event_name(), "Parsed event from message");
                }
            }
            Ok(Message::Ping(_)) => {
                trace!(bot_id = %bot_id_recv, "Received ping");
            }
            Ok(Message::Pong(_)) => {
                trace!(bot_id = %bot_id_recv, "Received pong");
            }
            Ok(Message::Close(_)) => {
                info!(bot_id = %bot_id_recv, "WebSocket connection closed by client");
                break;
            }
            Err(e) => {
                warn!(bot_id = %bot_id_recv, error = %e, "WebSocket error");
                handler.on_error(&bot_id_recv, &e.to_string()).await;
                break;
            }
        }
    }

    // Cleanup
    send_task.abort();

    // Remove from connections
    {
        let mut connections = state.connections.write().await;
        connections.remove(&bot_id);
    }

    // Notify disconnect
    state.handler.on_disconnect(&bot_id).await;
    info!(bot_id = %bot_id, "WebSocket connection closed");
}
