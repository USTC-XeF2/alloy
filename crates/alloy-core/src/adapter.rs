//! Adapter trait and bridge.
//!
//! Adapters bridge protocol implementations with the Alloy event system.
//! Each adapter implements the [`Adapter`] trait, which combines:
//! - **Protocol hooks**: Bot ID extraction, bot creation, message parsing
//! - **Lifecycle**: Start/shutdown management
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//!
//! # Architecture
//!
//! ```text
//! Transport ←→ AdapterBridge ←→ Adapter (protocol-specific)
//!                     ↕
//!                Bot lifecycle
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! impl Adapter for OneBotAdapter {
//!     async fn get_bot_id(&self, info: ConnectionInfo) -> TransportResult<String> {
//!         info.metadata.get("x-self-id").cloned()
//!             .ok_or(TransportError::BotIdMissing { reason: "missing".into() })
//!     }
//!
//!     fn create_bot(&self, bot_id: &str, conn: ConnectionHandle) -> BoxedBot {
//!         OneBotBot::new(bot_id, conn)
//!     }
//!
//!     async fn on_message(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent> {
//!         parse_onebot_event(data).ok()
//!     }
//!
//!     async fn on_start(&self, bridge: Arc<AdapterBridge>) -> AdapterResult<()> {
//!         if let Some(ws) = bridge.transport().ws_server() {
//!             let handle = ws.listen("0.0.0.0:8080", "/ws", bridge.clone()).await?;
//!             bridge.add_listener(handle).await;
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::bot::BoxedBot;
use crate::error::{AdapterResult, TransportError, TransportResult};
use crate::event::BoxedEvent;
use crate::transport::{ConnectionHandle, ConnectionInfo, ListenerHandle, TransportContext};

// =============================================================================
// Adapter Trait
// =============================================================================

/// The core adapter trait.
///
/// Adapters bridge protocol-specific implementations (like OneBot) with
/// the Alloy framework. Each adapter implements both protocol hooks
/// (bot ID extraction, bot creation, message parsing) and lifecycle management.
///
/// Protocol hooks are called by [`AdapterBridge`] automatically.
/// The adapter only needs to implement the protocol-specific logic.
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Extract a bot ID from connection metadata.
    ///
    /// Called when a new transport connection is established.
    /// Return the unique bot identifier derived from protocol-specific
    /// metadata (e.g., headers, query params).
    async fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String>;

    /// Create a bot instance for a new connection.
    ///
    /// Called after [`get_bot_id`](Self::get_bot_id) succeeds.
    /// The returned bot is automatically registered with the [`BotManager`].
    fn create_bot(&self, bot_id: &str, connection: ConnectionHandle) -> BoxedBot;

    /// Parse an incoming message into an event.
    ///
    /// Called when raw data is received from the transport.
    /// Return `None` for non-event messages (e.g., API responses).
    /// The bot is provided for protocol-specific handling
    /// (e.g., forwarding API responses to the bot instance).
    async fn on_message(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent>;

    /// Called when the adapter should start.
    ///
    /// Use the bridge to access transport capabilities and register listeners.
    ///
    /// ```rust,ignore
    /// async fn on_start(&self, bridge: Arc<AdapterBridge>) -> AdapterResult<()> {
    ///     if let Some(ws_server) = bridge.transport().ws_server() {
    ///         let handle = ws_server.listen("0.0.0.0:8080", "/ws", bridge.clone()).await?;
    ///         bridge.add_listener(handle).await;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn on_start(&self, bridge: Arc<AdapterBridge>) -> AdapterResult<()>;

    /// Called when the adapter is shutting down.
    async fn on_shutdown(&self, _bridge: Arc<AdapterBridge>) -> AdapterResult<()> {
        Ok(())
    }
}

/// A boxed adapter trait object.
pub type BoxedAdapter = Arc<dyn Adapter>;

/// Event dispatcher callback type.
///
/// Receives events and the associated bot, then distributes them to registered handlers.
/// Typically created by the runtime and passed to adapters via [`AdapterBridge`].
pub type Dispatcher = Arc<dyn Fn(BoxedEvent, BoxedBot) + Send + Sync>;

/// Trait for adapters that can be created from YAML configuration.
///
/// Separates compile-time concerns (`Config` type, `from_config()`)
/// from the object-safe [`Adapter`] trait.
pub trait ConfigurableAdapter: Adapter {
    /// The configuration type, deserialized from `alloy.yaml`.
    type Config: serde::de::DeserializeOwned + Default;

    /// Returns the adapter name used as the config key.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Creates an adapter instance from its deserialized configuration.
    fn from_config(config: Self::Config) -> Self
    where
        Self: Sized;
}

// =============================================================================
// Adapter Bridge
// =============================================================================

/// Bridge between the transport layer and an [`Adapter`].
///
/// Handles bot lifecycle and provides access to transport capabilities:
/// - **Bot lifecycle**: register, unregister, get, dispatch events
/// - **Transport access**: provides TransportContext for starting listeners/clients
/// - **Resource management**: tracks listeners and connections
///
/// Created by the runtime and passed to adapters via [`Adapter::on_start`].
/// Transport capabilities accept `Arc<AdapterBridge>` for callbacks.
///
/// Each `AdapterBridge` manages bots for a specific adapter instance.
pub struct AdapterBridge {
    adapter: Arc<dyn Adapter>,
    /// Active bots by ID.
    bots: RwLock<HashMap<String, BoxedBot>>,
    /// Event dispatcher callback (always requires bot).
    event_dispatcher: Dispatcher,
    /// Available transport capabilities.
    transport: TransportContext,
    /// Active listener handles (to keep them alive).
    listeners: RwLock<Vec<ListenerHandle>>,
    /// Active connection handles.
    connections: RwLock<HashMap<String, ConnectionHandle>>,
}

impl AdapterBridge {
    /// Creates a new adapter bridge.
    pub fn new(
        adapter: Arc<dyn Adapter>,
        event_dispatcher: Dispatcher,
        transport: TransportContext,
    ) -> Self {
        Self {
            adapter,
            bots: RwLock::new(HashMap::new()),
            event_dispatcher,
            transport,
            listeners: RwLock::new(Vec::new()),
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Returns a reference to the transport context.
    pub fn transport(&self) -> &TransportContext {
        &self.transport
    }

    /// Registers a listener handle (keeps it alive).
    pub async fn add_listener(&self, handle: ListenerHandle) {
        self.listeners.write().await.push(handle);
    }

    /// Registers a connection handle.
    pub async fn add_connection(&self, handle: ConnectionHandle) {
        self.connections
            .write()
            .await
            .insert(handle.id.clone(), handle);
    }

    /// Extracts a bot ID from connection metadata.
    ///
    /// Delegates to [`Adapter::get_bot_id`].
    pub async fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String> {
        self.adapter.get_bot_id(conn_info).await
    }

    /// Creates and registers a bot for this connection.
    ///
    /// Calls [`Adapter::create_bot`] then registers the bot.
    pub async fn create_bot(&self, bot_id: &str, connection: ConnectionHandle) {
        let bot = self.adapter.create_bot(bot_id, connection.clone());
        if let Err(e) = self.register(bot_id.to_string(), bot).await {
            warn!(bot_id = %bot_id, error = %e, "Failed to register bot");
        } else {
            debug!(
                bot_id = %bot_id,
                "Bot registered"
            );
        }
    }

    /// Called when data is received from a connection.
    pub async fn on_message(&self, bot_id: &str, data: &[u8]) -> Option<BoxedEvent> {
        let bot = if let Some(bot) = self.get_bot(bot_id).await {
            bot
        } else {
            warn!(bot_id = %bot_id, "Received message but bot not found");
            return None;
        };
        let event = self.adapter.on_message(&bot, data).await?;
        self.dispatch_event(bot_id, event.clone()).await;
        Some(event)
    }

    /// Called when a connection is closed.
    pub async fn on_disconnect(&self, bot_id: &str) {
        if let Some(bot) = self.get_bot(bot_id).await {
            bot.on_disconnect().await;
        }
        self.unregister(bot_id).await;
        info!(
            bot_id = %bot_id,
            "Connection closed"
        );
    }

    /// Called when a connection error occurs.
    pub async fn on_error(&self, bot_id: &str, error: &str) {
        warn!(bot_id = %bot_id, error = %error);
    }

    // =========================================================================
    // Bot Management (formerly BotManager methods)
    // =========================================================================

    /// Registers a new bot.
    async fn register(&self, id: String, bot: BoxedBot) -> TransportResult<()> {
        let mut bots = self.bots.write().await;
        if bots.contains_key(&id) {
            return Err(TransportError::BotAlreadyExists { id });
        }
        bots.insert(id, bot);
        Ok(())
    }

    /// Gets the bot instance by ID.
    pub async fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        let bots = self.bots.read().await;
        bots.get(id).cloned()
    }

    /// Unregisters a bot.
    async fn unregister(&self, id: &str) {
        let mut bots = self.bots.write().await;
        bots.remove(id);
    }

    /// Dispatches an event with the associated bot.
    async fn dispatch_event(&self, bot_id: &str, event: BoxedEvent) -> bool {
        if let Some(bot) = self.get_bot(bot_id).await {
            (self.event_dispatcher)(event, bot);
            return true;
        }
        warn!(bot_id = %bot_id, "Cannot dispatch event: bot not found");
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
}
