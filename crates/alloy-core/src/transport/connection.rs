//! Connection handling and lifecycle types.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::{TransportError, TransportResult};
use crate::event::BoxedEvent;

// =============================================================================
// Message Handler
// =============================================================================

/// A handler for incoming messages from a transport connection.
///
/// The handler receives raw bytes and returns parsed events (if any).
pub type MessageHandler = Arc<dyn Fn(&[u8]) -> Option<BoxedEvent> + Send + Sync>;

/// A handler for connection lifecycle events.
#[async_trait]
pub trait ConnectionHandler: Send + Sync {
    /// Called when a new connection is established.
    ///
    /// Returns a unique bot ID for this connection, or an error if connection setup fails.
    async fn on_connect(&self, conn_info: ConnectionInfo) -> TransportResult<String>;

    /// Called after connection is established and handle is available.
    ///
    /// This is where you can create and register bot instances.
    async fn on_ready(&self, _bot_id: &str, _connection: ConnectionHandle) {
        // Default implementation does nothing
    }

    /// Called when data is received from a connection.
    ///
    /// Returns an event to dispatch, if the data represents one.
    async fn on_message(&self, bot_id: &str, data: &[u8]) -> Option<BoxedEvent>;

    /// Called when a connection is closed.
    async fn on_disconnect(&self, bot_id: &str);

    /// Called when a connection error occurs.
    async fn on_error(&self, bot_id: &str, error: &str);
}

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

/// Boxed connection handler.
pub type BoxedConnectionHandler = Arc<dyn ConnectionHandler>;

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

/// Handle to a client connection.
///
/// Provides methods to interact with the connection.
#[derive(Debug, Clone)]
pub struct ConnectionHandle {
    /// Unique identifier for this connection (bot ID).
    pub id: String,
    /// Sender for outgoing messages.
    message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    /// Shutdown signal sender.
    shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
}

impl ConnectionHandle {
    /// Creates a new connection handle.
    pub fn new(
        id: impl Into<String>,
        message_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            message_tx,
            shutdown_tx: Arc::new(shutdown_tx),
        }
    }

    /// Sends a message through this connection.
    pub async fn send(&self, data: Vec<u8>) -> TransportResult<()> {
        self.message_tx
            .send(data)
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    /// Sends a JSON message.
    pub async fn send_json(&self, value: &Value) -> TransportResult<()> {
        let data = serde_json::to_vec(value)
            .map_err(|e| TransportError::SendFailed(format!("JSON serialization failed: {e}")))?;
        self.send(data).await
    }

    /// Closes this connection.
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
