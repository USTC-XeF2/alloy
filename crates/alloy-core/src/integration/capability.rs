//! Transport capability system for the Alloy framework.
//!
//! This module provides a capability-based approach for adapters to discover
//! and use available transport features at runtime.
//!
//! # Overview
//!
//! Instead of configuring transports upfront, adapters query available capabilities
//! and register handlers for the ones they need:
//!
//! ```rust,ignore
//! impl Adapter for MyAdapter {
//!     fn on_init(&self, ctx: &TransportContext) {
//!         // Try to get WebSocket server capability
//!         if let Some(ws_server) = ctx.get_capability::<dyn WsServerCapability>() {
//!             ws_server.listen("0.0.0.0:8080", self.message_handler());
//!         }
//!
//!         // Try to get WebSocket client capability  
//!         if let Some(ws_client) = ctx.get_capability::<dyn WsClientCapability>() {
//!             ws_client.connect("ws://127.0.0.1:9000", self.message_handler());
//!         }
//!     }
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots can join/leave at runtime:
//! - **Server transports**: New connections become bots, disconnections remove them
//! - **Client transports**: Configured endpoints auto-reconnect on disconnect

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::foundation::event::BoxedEvent;

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
    /// Returns a unique bot ID for this connection.
    async fn on_connect(&self, conn_info: ConnectionInfo) -> String;

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
// Transport Capabilities
// =============================================================================

/// WebSocket server capability.
///
/// Allows an adapter to listen for incoming WebSocket connections.
#[async_trait]
pub trait WsServerCapability: Send + Sync {
    /// Starts listening on the specified address.
    ///
    /// Each incoming connection will be handled by the provided handler.
    /// Returns a handle that can be used to stop the listener.
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: BoxedConnectionHandler,
    ) -> anyhow::Result<ListenerHandle>;

    /// Returns the default listen address.
    fn default_addr(&self) -> &str {
        "0.0.0.0:8080"
    }

    /// Returns the default path.
    fn default_path(&self) -> &str {
        "/ws"
    }
}

/// WebSocket client capability.
///
/// Allows an adapter to connect to WebSocket servers.
#[async_trait]
pub trait WsClientCapability: Send + Sync {
    /// Connects to a WebSocket server.
    ///
    /// The connection will auto-reconnect based on the retry config.
    /// Returns a handle that can be used to manage the connection.
    async fn connect(
        &self,
        url: &str,
        handler: BoxedConnectionHandler,
        config: ClientConfig,
    ) -> anyhow::Result<ConnectionHandle>;
}

/// HTTP server capability.
///
/// Allows an adapter to receive HTTP callbacks.
#[async_trait]
pub trait HttpServerCapability: Send + Sync {
    /// Starts an HTTP server on the specified address.
    ///
    /// The handler will be called for each incoming request.
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: BoxedConnectionHandler,
    ) -> anyhow::Result<ListenerHandle>;
}

/// HTTP client capability.
///
/// Allows an adapter to make HTTP requests.
#[async_trait]
pub trait HttpClientCapability: Send + Sync {
    /// Sends an HTTP POST request with JSON body.
    async fn post_json(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;

    /// Sends an HTTP GET request.
    async fn get(&self, url: &str) -> anyhow::Result<serde_json::Value>;
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
    pub async fn send(&self, data: Vec<u8>) -> anyhow::Result<()> {
        self.message_tx
            .send(data)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send message: {e}"))
    }

    /// Sends a JSON message.
    pub async fn send_json(&self, value: &serde_json::Value) -> anyhow::Result<()> {
        let data = serde_json::to_vec(value)?;
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

// =============================================================================
// Transport Context
// =============================================================================

/// Context for adapter initialization.
///
/// Provides access to available transport capabilities.
#[derive(Clone)]
pub struct TransportContext {
    ws_server: Option<Arc<dyn WsServerCapability>>,
    ws_client: Option<Arc<dyn WsClientCapability>>,
    http_server: Option<Arc<dyn HttpServerCapability>>,
    http_client: Option<Arc<dyn HttpClientCapability>>,
}

impl TransportContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self {
            ws_server: None,
            ws_client: None,
            http_server: None,
            http_client: None,
        }
    }

    /// Registers the WebSocket server capability.
    pub fn with_ws_server(mut self, cap: Arc<dyn WsServerCapability>) -> Self {
        self.ws_server = Some(cap);
        self
    }

    /// Registers the WebSocket client capability.
    pub fn with_ws_client(mut self, cap: Arc<dyn WsClientCapability>) -> Self {
        self.ws_client = Some(cap);
        self
    }

    /// Registers the HTTP server capability.
    pub fn with_http_server(mut self, cap: Arc<dyn HttpServerCapability>) -> Self {
        self.http_server = Some(cap);
        self
    }

    /// Registers the HTTP client capability.
    pub fn with_http_client(mut self, cap: Arc<dyn HttpClientCapability>) -> Self {
        self.http_client = Some(cap);
        self
    }

    /// Sets the WebSocket server capability.
    pub fn set_ws_server(&mut self, cap: Arc<dyn WsServerCapability>) {
        self.ws_server = Some(cap);
    }

    /// Sets the WebSocket client capability.
    pub fn set_ws_client(&mut self, cap: Arc<dyn WsClientCapability>) {
        self.ws_client = Some(cap);
    }

    /// Sets the HTTP server capability.
    pub fn set_http_server(&mut self, cap: Arc<dyn HttpServerCapability>) {
        self.http_server = Some(cap);
    }

    /// Sets the HTTP client capability.
    pub fn set_http_client(&mut self, cap: Arc<dyn HttpClientCapability>) {
        self.http_client = Some(cap);
    }

    /// Gets the WebSocket server capability if available.
    pub fn ws_server(&self) -> Option<&Arc<dyn WsServerCapability>> {
        self.ws_server.as_ref()
    }

    /// Gets the WebSocket client capability if available.
    pub fn ws_client(&self) -> Option<&Arc<dyn WsClientCapability>> {
        self.ws_client.as_ref()
    }

    /// Gets the HTTP server capability if available.
    pub fn http_server(&self) -> Option<&Arc<dyn HttpServerCapability>> {
        self.http_server.as_ref()
    }

    /// Gets the HTTP client capability if available.
    pub fn http_client(&self) -> Option<&Arc<dyn HttpClientCapability>> {
        self.http_client.as_ref()
    }

    /// Checks if WebSocket server is available.
    pub fn has_ws_server(&self) -> bool {
        self.ws_server.is_some()
    }

    /// Checks if WebSocket client is available.
    pub fn has_ws_client(&self) -> bool {
        self.ws_client.is_some()
    }

    /// Checks if HTTP server is available.
    pub fn has_http_server(&self) -> bool {
        self.http_server.is_some()
    }

    /// Checks if HTTP client is available.
    pub fn has_http_client(&self) -> bool {
        self.http_client.is_some()
    }
}

impl Default for TransportContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Bot Manager
// =============================================================================

/// Manages dynamically connected bots.
///
/// Bots can join and leave at any time:
/// - Server: new connections add bots, disconnections remove them
/// - Client: configured endpoints create bots, with auto-reconnect on disconnect
pub struct BotManager {
    /// Active bots by ID.
    bots: RwLock<HashMap<String, BotEntry>>,
    /// Event dispatcher callback (always requires bot).
    event_dispatcher: Arc<dyn Fn(BoxedEvent, crate::integration::bot::BoxedBot) + Send + Sync>,
}

/// Entry for a managed bot.
struct BotEntry {
    /// Bot ID.
    id: String,
    /// Connection handle for sending messages.
    connection: ConnectionHandle,
    /// Adapter name that owns this bot.
    adapter: String,
    /// Bot metadata.
    metadata: HashMap<String, String>,
    /// Bot instance (if available).
    bot: Option<crate::integration::bot::BoxedBot>,
}

impl std::fmt::Debug for BotEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotEntry")
            .field("id", &self.id)
            .field("connection", &self.connection)
            .field("adapter", &self.adapter)
            .field("metadata", &self.metadata)
            .field("has_bot", &self.bot.is_some())
            .finish()
    }
}

impl BotManager {
    /// Creates a new bot manager with the event dispatcher.
    ///
    /// The dispatcher callback receives both the event and the associated bot.
    pub fn new(
        event_dispatcher: Arc<dyn Fn(BoxedEvent, crate::integration::bot::BoxedBot) + Send + Sync>,
    ) -> Self {
        Self {
            bots: RwLock::new(HashMap::new()),
            event_dispatcher,
        }
    }

    /// Registers a new bot.
    pub async fn register(
        &self,
        id: String,
        connection: ConnectionHandle,
        adapter: String,
    ) -> anyhow::Result<()> {
        let mut bots = self.bots.write().await;
        if bots.contains_key(&id) {
            anyhow::bail!("Bot with ID '{id}' already exists");
        }
        bots.insert(
            id.clone(),
            BotEntry {
                id,
                connection,
                adapter,
                metadata: HashMap::new(),
                bot: None,
            },
        );
        Ok(())
    }

    /// Registers a new bot with a Bot instance.
    pub async fn register_with_bot(
        &self,
        id: String,
        connection: ConnectionHandle,
        adapter: String,
        bot: crate::integration::bot::BoxedBot,
    ) -> anyhow::Result<()> {
        let mut bots = self.bots.write().await;
        if bots.contains_key(&id) {
            anyhow::bail!("Bot with ID '{id}' already exists");
        }
        bots.insert(
            id.clone(),
            BotEntry {
                id,
                connection,
                adapter,
                metadata: HashMap::new(),
                bot: Some(bot),
            },
        );
        Ok(())
    }

    /// Sets the bot instance for an already registered bot.
    pub async fn set_bot(
        &self,
        id: &str,
        bot: crate::integration::bot::BoxedBot,
    ) -> anyhow::Result<()> {
        let mut bots = self.bots.write().await;
        let entry = bots
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Bot '{id}' not found"))?;
        entry.bot = Some(bot);
        Ok(())
    }

    /// Gets the bot instance by ID.
    pub async fn get_bot(&self, id: &str) -> Option<crate::integration::bot::BoxedBot> {
        let bots = self.bots.read().await;
        bots.get(id).and_then(|e| e.bot.clone())
    }

    /// Unregisters a bot.
    pub async fn unregister(&self, id: &str) -> Option<ConnectionHandle> {
        let mut bots = self.bots.write().await;
        bots.remove(id).map(|e| e.connection)
    }

    /// Gets a connection handle for a bot.
    pub async fn get_connection(&self, id: &str) -> Option<ConnectionHandle> {
        let bots = self.bots.read().await;
        bots.get(id).map(|e| e.connection.clone())
    }

    /// Dispatches an event with the associated bot.
    ///
    /// If the event has a `bot_id()` and a bot instance is registered,
    /// this will dispatch the event with that bot.
    ///
    /// # Returns
    ///
    /// `true` if the event was dispatched, `false` if the bot was not found.
    pub async fn dispatch_event(&self, event: BoxedEvent) -> bool {
        if let Some(bot_id) = event.inner().bot_id() {
            if let Some(bot) = self.get_bot(bot_id).await {
                (self.event_dispatcher)(event, bot);
                return true;
            }
            tracing::warn!(bot_id = %bot_id, "Cannot dispatch event: bot not found");
        } else {
            tracing::warn!(event = %event.event_name(), "Cannot dispatch event: no bot_id in event");
        }
        false
    }

    /// Returns the IDs of all active bots.
    pub async fn bot_ids(&self) -> Vec<String> {
        let bots = self.bots.read().await;
        bots.keys().cloned().collect()
    }

    /// Returns the count of active bots.
    pub async fn bot_count(&self) -> usize {
        let bots = self.bots.read().await;
        bots.len()
    }

    /// Sends a message to a specific bot.
    pub async fn send_to(&self, bot_id: &str, data: Vec<u8>) -> anyhow::Result<()> {
        let bots = self.bots.read().await;
        let bot = bots
            .get(bot_id)
            .ok_or_else(|| anyhow::anyhow!("Bot '{bot_id}' not found"))?;
        bot.connection.send(data).await
    }

    /// Broadcasts a message to all bots.
    pub async fn broadcast(&self, data: Vec<u8>) -> Vec<anyhow::Result<()>> {
        let bots = self.bots.read().await;
        let mut results = Vec::new();
        for bot in bots.values() {
            results.push(bot.connection.send(data.clone()).await);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_context() {
        let ctx = TransportContext::new();
        assert!(!ctx.has_ws_server());
        assert!(!ctx.has_ws_client());
        assert!(!ctx.has_http_server());
        assert!(!ctx.has_http_client());
    }

    #[test]
    fn test_client_config() {
        let config = ClientConfig::default();
        assert!(config.auto_reconnect);
        assert!(config.max_retries.is_none());

        let config = ClientConfig::no_reconnect();
        assert!(!config.auto_reconnect);
    }
}
