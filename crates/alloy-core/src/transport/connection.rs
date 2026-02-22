//! Connection handling and lifecycle types.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{TransportError, TransportResult};
use crate::event::BoxedEvent;

/// Type-erased async function that performs an HTTP POST and returns JSON.
///
/// The URL and any authentication (e.g. Bearer token) are captured when the
/// closure is constructed by the transport layer.  Callers only supply the
/// request body.
pub type PostJsonFn = Arc<
    dyn Fn(Value) -> Pin<Box<dyn std::future::Future<Output = TransportResult<Value>> + Send>>
        + Send
        + Sync,
>;

// =============================================================================
// Message Handler
// =============================================================================

/// A handler for incoming messages from a transport connection.
///
/// The handler receives raw bytes and returns parsed events (if any).
pub type MessageHandler = Arc<dyn Fn(&[u8]) -> Option<BoxedEvent> + Send + Sync>;

/// Information about a connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Remote address (if available).
    pub remote_addr: Option<String>,
    /// Connection protocol (ws, http, etc.).
    pub protocol: String,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl ConnectionInfo {
    /// Creates new connection info.
    pub fn new(protocol: impl Into<String>) -> Self {
        Self {
            remote_addr: None,
            protocol: protocol.into(),
            metadata: HashMap::new(),
        }
    }

    /// Sets the remote address.
    pub fn with_remote_addr(mut self, addr: impl Into<String>) -> Self {
        self.remote_addr = Some(addr.into());
        self
    }

    /// Adds metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// =============================================================================
// Handles
// =============================================================================

/// Handle to a listener (server).
///
/// Dropping this handle stops the listener.
#[derive(Debug)]
pub struct ListenerHandle {
    /// Unique identifier for this listener.
    pub id: String,
    /// Shutdown signal sender.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ListenerHandle {
    /// Creates a new listener handle.
    pub fn new(id: impl Into<String>, shutdown_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            id: id.into(),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Stops the listener.
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

// =============================================================================
// ConnectionKind — transport-specific data
// =============================================================================

/// Transport-specific data carried by a [`ConnectionHandle`].
///
/// Each variant holds *only* what that transport actually needs — no optional
/// fields, no dummy channels, no `HashMap` catch-all.
#[derive(Clone)]
pub enum ConnectionKind {
    /// WebSocket connection (outbound dial or inbound accept — identical after handshake).
    Ws {
        /// Channel to the WS write loop; send serialised frames here.
        message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    },
    /// HTTP outbound API client.
    ///
    /// All connection parameters (URL, auth) are baked into `post_json` at
    /// construction time.  The adapter only needs to call `post_json(body)`.
    HttpClient {
        /// Type-erased async POST function provided by the transport layer.
        /// URL and authentication are captured inside the closure.
        post_json: PostJsonFn,
    },
    /// HTTP inbound webhook server.
    ///
    /// Events arrive via POST.  `message_tx` is an optional hook for future
    /// SSE / response-channel delivery; it is not used for API calls.
    HttpServer {
        /// Outgoing message queue (reserved for future SSE / push mechanisms).
        message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    },
}

impl std::fmt::Debug for ConnectionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionKind::Ws { .. } => write!(f, "Ws"),
            ConnectionKind::HttpClient { .. } => write!(f, "HttpClient"),
            ConnectionKind::HttpServer { .. } => write!(f, "HttpServer"),
        }
    }
}

// =============================================================================
// ConnectionHandle
// =============================================================================

/// Handle to an active bot connection.
///
/// The `kind` field carries all transport-specific data; the handle itself
/// only contains fields common to every connection.
#[derive(Clone, Debug)]
pub struct ConnectionHandle {
    /// Unique identifier for this connection (bot ID).
    pub id: String,
    /// Transport-specific data for this connection.
    pub kind: ConnectionKind,
    /// Shutdown signal sender.
    shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
}

impl ConnectionHandle {
    // -------------------------------------------------------------------------
    // Constructors (one per transport kind)
    // -------------------------------------------------------------------------

    /// Creates a handle for a WebSocket connection.
    ///
    /// Works for both outbound (client) and inbound (server) connections;
    /// after the handshake, their behavior is identical.
    pub fn new_ws(
        id: impl Into<String>,
        message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::Ws { message_tx },
            shutdown_tx: Arc::new(shutdown_tx),
        }
    }

    /// Creates a handle for an HTTP outbound API client connection.
    ///
    /// The `post_json` closure must already capture the target URL and any
    /// required authentication; the handle itself stores nothing protocol-specific.
    pub fn new_http_client(
        id: impl Into<String>,
        post_json: PostJsonFn,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::HttpClient { post_json },
            shutdown_tx: Arc::new(shutdown_tx),
        }
    }

    /// Creates a handle for an HTTP inbound webhook server connection.
    pub fn new_http_server(
        id: impl Into<String>,
        message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::HttpServer { message_tx },
            shutdown_tx: Arc::new(shutdown_tx),
        }
    }

    // -------------------------------------------------------------------------
    // Common accessors
    // -------------------------------------------------------------------------

    /// Sends raw bytes over this connection.
    ///
    /// Valid for `WsClient`, `WsServer`, and `HttpServer` connections.
    /// Returns [`TransportError::SendFailed`] for `HttpClient` connections
    /// (those issue API calls via [`ConnectionKind::HttpClient::http`] instead).
    pub async fn send(&self, data: Vec<u8>) -> TransportResult<()> {
        let tx = match &self.kind {
            ConnectionKind::Ws { message_tx } | ConnectionKind::HttpServer { message_tx } => {
                message_tx
            }
            ConnectionKind::HttpClient { .. } => {
                return Err(TransportError::SendFailed(
                    "HTTP client connections do not use a raw send channel; \
                     use HttpClientCapability::post_json instead"
                        .into(),
                ));
            }
        };
        tx.send(data)
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    /// Sends a JSON value over this connection.
    pub async fn send_json(&self, value: &Value) -> TransportResult<()> {
        let data = serde_json::to_vec(value)
            .map_err(|e| TransportError::SendFailed(format!("JSON serialization failed: {e}")))?;
        self.send(data).await
    }

    /// Signals the transport loop to shut down this connection.
    pub fn close(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

// =============================================================================
// Client Configuration
// =============================================================================

/// Configuration for client connections.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Whether to automatically reconnect on disconnect.
    pub auto_reconnect: bool,
    /// Maximum number of reconnection attempts (None = infinite).
    pub max_retries: Option<u32>,
    /// Initial delay between reconnection attempts.
    pub initial_delay: std::time::Duration,
    /// Maximum delay between reconnection attempts.
    pub max_delay: std::time::Duration,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
    /// Optional access token for authentication.
    pub access_token: Option<String>,
    /// Heartbeat interval (for WebSocket).
    pub heartbeat_interval: Option<std::time::Duration>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            max_retries: None, // Infinite retries
            initial_delay: std::time::Duration::from_secs(1),
            max_delay: std::time::Duration::from_secs(60),
            backoff_multiplier: 2.0,
            access_token: None,
            heartbeat_interval: Some(std::time::Duration::from_secs(30)),
        }
    }
}

impl ClientConfig {
    /// Creates a new client config with auto-reconnect disabled.
    pub fn no_reconnect() -> Self {
        Self {
            auto_reconnect: false,
            ..Default::default()
        }
    }

    /// Sets the access token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Sets the maximum retry count.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = Some(max);
        self
    }
}
