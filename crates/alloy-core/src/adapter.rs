//! Adapter trait and bridge.
//!
//! Adapters bridge protocol implementations with the Alloy event system.
//! Each adapter implements the [`Adapter`] trait, which combines:
//! - **Protocol hooks**: Bot ID extraction, bot creation, message parsing
//! - **Lifecycle**: Start/shutdown management
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//! Its methods are organized into three traits to clarify who may call what:
//!
//! | Trait | Caller | Methods |
//! |---|---|---|
//! | [`ConnectionHandler`](crate::transport::ConnectionHandler) | transport 层 | `get_bot_id`, `create_bot`, `on_message`, `on_disconnect`, `on_error` |
//! | [`AdapterContext`] | adapter 自身 | `transport`, `add_listener`, `add_connection`, `get_bot` |
//! | (直接方法) | runtime | `on_start`, `on_shutdown`, `bot_ids`, `bot_count` |
//!
//! # Architecture
//!
//! ```text
//! Transport ←→ Arc<dyn ConnectionHandler>
//!                     ↕ (implemented by AdapterBridge)
//! Adapter  ←→ Arc<dyn AdapterContext>
//!                     ↕
//! Runtime  ←→ Arc<AdapterBridge>
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
//!     async fn on_start(&self, ctx: Arc<dyn AdapterContext>) -> AdapterResult<()> {
//!         if let Some(ws) = ctx.transport().ws_server() {
//!             let handle = ws.listen("0.0.0.0:8080", "/ws", ctx.as_connection_handler()).await?;
//!             ctx.add_listener(handle).await;
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
use crate::error::{AdapterResult, TransportResult};
use crate::event::BoxedEvent;
use crate::transport::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, TransportContext,
};

// =============================================================================
// AdapterContext Trait — called by adapter implementations
// =============================================================================

/// Interface exposed to [`Adapter`] implementations during `on_start`.
///
/// Adapters receive `Arc<dyn AdapterContext>` and use it to access transport
/// capabilities, register listeners/connections, and query active bots.
#[async_trait]
pub trait AdapterContext: Send + Sync {
    /// Returns a reference to the transport capability context.
    fn transport(&self) -> &TransportContext;

    /// Registers a listener handle, keeping it alive for the adapter's lifetime.
    async fn add_listener(&self, handle: ListenerHandle);

    /// Registers an outbound connection handle.
    async fn add_connection(&self, handle: ConnectionHandle);

    /// Returns a bot by ID, or `None` if not found.
    async fn get_bot(&self, id: &str) -> Option<BoxedBot>;

    /// Casts this context to a `ConnectionHandler` reference for passing to
    /// transport capabilities (e.g., `ws_server.listen(..., ctx.as_connection_handler())`).
    fn as_connection_handler(self: Arc<Self>) -> Arc<dyn ConnectionHandler>;
}

// =============================================================================
// Adapter Trait
// =============================================================================

/// The core adapter trait.
///
/// An adapter provides the protocol-specific logic:
/// - **Protocol hooks**: Bot ID extraction, bot creation, message parsing
///   — these are called internally by [`AdapterBridge`] via [`TransportCallback`].
/// - **Lifecycle**: `on_start` / `on_shutdown`
///   — receives [`AdapterContext`] for transport access.
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
    fn create_bot(&self, bot_id: &str, connection: ConnectionHandle) -> BoxedBot;

    /// Parse an incoming message into an event.
    ///
    /// Called when raw data is received from the transport.
    /// Return `None` for non-event messages (e.g., API responses).
    /// The bot is provided for protocol-specific handling
    /// (e.g., forwarding API responses to the bot instance).
    async fn parse_event(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent>;

    /// Called when the adapter should start.
    ///
    /// Use the context to access transport capabilities and register listeners.
    ///
    /// ```rust,ignore
    /// async fn on_start(&self, ctx: Arc<dyn AdapterContext>) -> AdapterResult<()> {
    ///     if let Some(ws_server) = ctx.transport().ws_server() {
    ///         let handle = ws_server.listen("0.0.0.0:8080", "/ws", ctx.as_connection_handler()).await?;
    ///         ctx.add_listener(handle).await;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn on_start(&self, ctx: Arc<dyn AdapterContext>) -> AdapterResult<()>;

    /// Called when the adapter is shutting down.
    async fn on_shutdown(&self, _ctx: Arc<dyn AdapterContext>) -> AdapterResult<()> {
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

/// Central bridge that wires together the runtime, the transport layer, and an adapter.
///
/// - Implements [`TransportCallback`] — transport implementations call it when
///   connections are established or data arrives.
/// - Implements [`AdapterContext`] — adapters call it during `on_start` to register
///   listeners and access transport capabilities.
/// - Exposes runtime-facing methods (`on_start`, `on_shutdown`, `bot_ids`, `bot_count`)
///   directly, since the runtime holds `Arc<AdapterBridge>`.
///
/// Each `AdapterBridge` manages bots for exactly one adapter instance.
pub struct AdapterBridge {
    adapter: Arc<dyn Adapter>,
    /// Active bots by ID.
    bots: RwLock<HashMap<String, BoxedBot>>,
    /// Event dispatcher callback.
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

    // =========================================================================
    // Runtime-facing methods
    // =========================================================================

    /// Starts the adapter (delegates to [`Adapter::on_start`]).
    pub async fn on_start(self: &Arc<Self>) -> AdapterResult<()> {
        let ctx: Arc<dyn AdapterContext> = self.clone();
        self.adapter.on_start(ctx).await
    }

    /// Shuts down the adapter (delegates to [`Adapter::on_shutdown`]).
    pub async fn on_shutdown(self: &Arc<Self>) -> AdapterResult<()> {
        let ctx: Arc<dyn AdapterContext> = self.clone();
        self.adapter.on_shutdown(ctx).await
    }

    /// Returns the IDs of all active bots.
    pub async fn bot_ids(&self) -> Vec<String> {
        self.bots.read().await.keys().cloned().collect()
    }

    /// Returns the count of active bots.
    pub async fn bot_count(&self) -> usize {
        self.bots.read().await.len()
    }
}

// =============================================================================
// ConnectionHandler impl — called by transport layer
// =============================================================================

#[async_trait]
impl ConnectionHandler for AdapterBridge {
    async fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String> {
        self.adapter.get_bot_id(conn_info).await
    }

    async fn create_bot(&self, bot_id: &str, connection: ConnectionHandle) {
        // Check if bot already exists before creating
        {
            let bots = self.bots.read().await;
            if bots.contains_key(bot_id) {
                warn!(bot_id = %bot_id, "Bot already exists, not registering");
                return;
            }
        }

        let bot = self.adapter.create_bot(bot_id, connection);
        let mut bots = self.bots.write().await;
        bots.insert(bot_id.to_string(), bot);
        debug!(bot_id = %bot_id, "Bot registered");
    }

    async fn on_message(&self, bot_id: &str, data: &[u8]) -> Option<BoxedEvent> {
        let bot = self.bots.read().await.get(bot_id).cloned()?;
        let event = self.adapter.parse_event(&bot, data).await?;
        (self.event_dispatcher)(event.clone(), bot);
        Some(event)
    }

    async fn on_disconnect(&self, bot_id: &str) {
        if let Some(bot) = self.bots.read().await.get(bot_id).cloned() {
            bot.on_disconnect().await;
        }
        self.bots.write().await.remove(bot_id);
        info!(bot_id = %bot_id, "Connection closed");
    }

    async fn on_error(&self, bot_id: &str, error: &str) {
        warn!(bot_id = %bot_id, error = %error);
    }
}

// =============================================================================
// AdapterContext impl — called by adapter implementations
// =============================================================================

#[async_trait]
impl AdapterContext for AdapterBridge {
    fn transport(&self) -> &TransportContext {
        &self.transport
    }

    async fn add_listener(&self, handle: ListenerHandle) {
        self.listeners.write().await.push(handle);
    }

    async fn add_connection(&self, handle: ConnectionHandle) {
        self.connections
            .write()
            .await
            .insert(handle.id.clone(), handle);
    }

    async fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        self.bots.read().await.get(id).cloned()
    }

    fn as_connection_handler(self: Arc<Self>) -> Arc<dyn ConnectionHandler> {
        self
    }
}
