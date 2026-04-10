//! Connection handling and lifecycle types.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::TransportResult;

/// Type-erased async function that performs an HTTP POST and returns JSON.
///
/// Type-erased async POST function for making JSON API calls.
///
/// The base URL and authentication (e.g. Bearer token) are captured when the
/// closure is constructed by the transport layer. Callers supply the endpoint
/// (relative path) and request body.
pub type PostJsonFn =
    Arc<dyn Fn(&str, Value) -> BoxFuture<'static, TransportResult<Value>> + Send + Sync>;

/// Information about a connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Remote address.
    pub remote_addr: SocketAddr,
    /// Headers.
    pub headers: HashMap<String, String>,
    /// Request body.
    pub body: Option<Vec<u8>>,
}

impl ConnectionInfo {
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    pub fn check_authorization(&self, expected_token: &str) -> bool {
        if let Some(auth_header) = self.headers.get("authorization")
            && let Some(token) = auth_header.to_lowercase().strip_prefix("bearer ")
        {
            token == expected_token
        } else {
            false
        }
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
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
    }
}

// =============================================================================
// Sender — transport-specific data
// =============================================================================

/// Transport-specific data carried by a [`ConnectionHandle`].
///
/// Each variant represents a transport that has **outbound send capability**.
/// Receive-only connections (e.g. HTTP server webhook) carry no sender and
/// store `None` in [`ConnectionHandle::sender`].
#[derive(Clone)]
pub enum Sender {
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
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sender::Ws { .. } => write!(f, "Ws"),
            Sender::HttpClient { .. } => write!(f, "HttpClient"),
        }
    }
}

// =============================================================================
// ConnectionHandle
// =============================================================================

/// Handle to an active bot connection.
///
/// `sender` carries the optional outbound-send capability for this bot.
/// A bot that only receives events (e.g. HTTP server webhook) will have
/// `sender == None`; a bot with a send channel (WS / HTTP-client) will have
/// a [`Some`] variant.
#[derive(Debug, Clone)]
pub struct ConnectionHandle {
    /// Optional outbound send capability.
    /// `None` ⇒ receive-only; `Some` ⇒ can also send API calls.
    pub(crate) sender: Option<Sender>,
    /// Cancellation token for graceful shutdown.
    pub(crate) shutdown_token: CancellationToken,
}

impl ConnectionHandle {
    /// Returns the connection sender, if any.
    pub fn sender(&self) -> Option<&Sender> {
        self.sender.as_ref()
    }

    /// Signals the transport loop to shut down this connection.
    pub fn close(self) {
        self.shutdown_token.cancel();
    }
}
