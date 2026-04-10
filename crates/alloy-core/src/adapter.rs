//! Adapter trait definitions.
//!
//! Adapters bridge protocol implementations with the Alloy event system.
//! Each adapter implements the [`Adapter`] trait, which combines:
//! - **Protocol hooks**: Bot ID extraction, bot creation, message parsing
//! - **Lifecycle**: Start/shutdown management

use std::sync::Arc;

use crate::bot::Bot;
use crate::error::AdapterResult;
use crate::event::BoxedEvent;
use crate::transport::{ConnectionHandle, ConnectionHandler, TransportContext};

// =============================================================================
// Adapter Trait
// =============================================================================

/// The core adapter trait.
///
/// An adapter provides the protocol-specific logic:
/// - **Protocol hooks**: Bot creation, message parsing
///   — these are called internally by [`AdapterBridge`] via [`TransportCallback`].
/// - **Lifecycle**: `on_start` / `on_shutdown`
///   — `on_start` receives transport capabilities and connection handler,
///   and `on_shutdown` runs as a parameterless hook.
pub trait Adapter: Send + Sync + 'static {
    /// The adapter name.
    const NAME: &'static str;

    /// The configuration type, deserialized from `alloy.toml`.
    type Config: serde::de::DeserializeOwned + Default;

    /// The bot type associated with this adapter.
    type Bot: Bot;

    /// Creates an adapter instance from its deserialized configuration.
    fn from_config(config: Self::Config) -> Self;

    /// Create a bot instance for a new connection.
    ///
    /// Called when transport layer resolves a bot ID and creates/registers
    /// a new connection handle.
    fn create_bot(&self, bot_id: &str, connection: &ConnectionHandle) -> Self::Bot;

    /// Parse an incoming message into an event.
    ///
    /// Called when raw data is received from the transport.
    /// Return `None` for non-event messages (e.g., API responses).
    /// The bot is provided for protocol-specific handling
    /// (e.g., forwarding API responses to the bot instance).
    fn on_message(
        &self,
        bot: &Self::Bot,
        data: &[u8],
    ) -> impl Future<Output = Option<BoxedEvent>> + Send;

    /// Called when the adapter should start.
    ///
    /// Use `transport` to access capabilities and `connection_handler` to
    /// register listeners.
    ///
    /// ```rust,ignore
    /// async fn on_start(
    ///     &self,
    ///     transport: TransportContext,
    ///     connection_handler: Arc<dyn ConnectionHandler>,
    /// ) -> AdapterResult<()> {
    ///     if let Some(ws_server) = transport.ws_server() {
    ///         let config = WsServerConfig::new("0.0.0.0", 8080, "/ws");
    ///         ws_server(config, connection_handler).await?;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    fn on_start(
        &self,
        transport: TransportContext,
        connection_handler: Arc<dyn ConnectionHandler>,
    ) -> impl Future<Output = AdapterResult<()>> + Send;
}
