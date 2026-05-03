//! Connection handling and lifecycle types.
//!
//! Defines [`Sender`] (transport-specific outbound capability), [`ConnectionInfo`]
//! (inbound connection metadata), [`ListenerHandle`] (server lifecycle handle),
//! and [`HttpRequestFn`] (type-erased HTTP client closure).

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use futures::future::BoxFuture;
use http::header::{AUTHORIZATION, AsHeaderName};
use http::{HeaderMap, Method};
use tokio::sync::{mpsc, watch};

use crate::error::TransportResult;

/// Type-erased async function that performs an HTTP request and returns raw bytes.
///
/// The full URL and authentication (e.g. Bearer token) are captured when the
/// closure is constructed by the transport layer. Callers supply the HTTP method,
/// endpoint (relative path), and request body.
pub type HttpRequestFn =
    Arc<dyn Fn(Method, &str, Bytes) -> BoxFuture<'static, TransportResult<Bytes>> + Send + Sync>;

/// Information about a connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Remote address.
    pub remote_addr: SocketAddr,
    /// Headers.
    pub headers: HeaderMap,
    /// Request body.
    pub body: Option<Bytes>,
}

impl ConnectionInfo {
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            headers: HeaderMap::new(),
            body: None,
        }
    }

    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_body(mut self, body: Bytes) -> Self {
        self.body = Some(body);
        self
    }

    pub fn get_header(&self, name: impl AsHeaderName) -> Option<&str> {
        self.headers.get(name).and_then(|h| h.to_str().ok())
    }

    pub fn check_authorization(&self, expected_token: &str) -> bool {
        if let Some(auth_header) = self.get_header(AUTHORIZATION)
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
    /// Cancellation receiver that, when dropped, signals shutdown.
    _shutdown_rx: watch::Receiver<()>,
}

impl ListenerHandle {
    /// Creates a new listener handle.
    pub fn new(id: impl Into<String>, shutdown_rx: watch::Receiver<()>) -> Self {
        Self {
            id: id.into(),
            _shutdown_rx: shutdown_rx,
        }
    }
}

// =============================================================================
// Sender — transport-specific data
// =============================================================================

/// Transport-specific data carried by a bot.
///
/// Each variant represents a transport that has **outbound send capability**.
/// Receive-only connections (e.g. HTTP server webhook) carry no sender.
#[derive(Clone)]
pub enum Sender {
    /// WebSocket connection (outbound dial or inbound accept — identical after handshake).
    Ws {
        /// Channel to the WS write loop; send serialised frames here.
        message_tx: mpsc::Sender<Bytes>,
    },
    /// HTTP outbound API client.
    ///
    /// All connection parameters (URL, auth) are baked into `http_request` at
    /// construction time.  The adapter only needs to call `http_request(method, endpoint, body)`.
    HttpClient {
        /// Type-erased async HTTP request function.
        http_request: HttpRequestFn,
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
