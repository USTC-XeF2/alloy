//! Connection handling and lifecycle types.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::TransportResult;

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
    /// Cancellation token for graceful shutdown.
    shutdown_token: CancellationToken,
}

impl ListenerHandle {
    /// Creates a new listener handle.
    pub fn new(id: impl Into<String>, shutdown_token: CancellationToken) -> Self {
        Self {
            id: id.into(),
            shutdown_token,
        }
    }

    /// Stops the listener.
    pub fn stop(self) {
        self.shutdown_token.cancel();
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
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
        message_tx: mpsc::Sender<Vec<u8>>,
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
        message_tx: mpsc::Sender<Vec<u8>>,
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
    /// Cancellation token for graceful shutdown.
    shutdown_token: CancellationToken,
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
        message_tx: mpsc::Sender<Vec<u8>>,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::Ws { message_tx },
            shutdown_token,
        }
    }

    /// Creates a handle for an HTTP outbound API client connection.
    ///
    /// The `post_json` closure must already capture the target URL and any
    /// required authentication; the handle itself stores nothing protocol-specific.
    pub fn new_http_client(
        id: impl Into<String>,
        post_json: PostJsonFn,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::HttpClient { post_json },
            shutdown_token,
        }
    }

    /// Creates a handle for an HTTP inbound webhook server connection.
    pub fn new_http_server(
        id: impl Into<String>,
        message_tx: mpsc::Sender<Vec<u8>>,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ConnectionKind::HttpServer { message_tx },
            shutdown_token,
        }
    }

    /// Signals the transport loop to shut down this connection.
    pub fn close(self) {
        self.shutdown_token.cancel();
    }
}
