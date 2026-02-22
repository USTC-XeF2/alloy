//! Adapter trait definitions.
//!
//! Adapters bridge protocol implementations with the Alloy event system.
//! Each adapter implements the [`Adapter`] trait, which combines:
//! - **Protocol hooks**: Bot ID extraction, bot creation, message parsing
//! - **Lifecycle**: Start/shutdown management
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
//!     async fn parse_event(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent> {
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

use std::sync::Arc;

use async_trait::async_trait;

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
    fn as_connection_handler(&self) -> Arc<dyn ConnectionHandler>;
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
