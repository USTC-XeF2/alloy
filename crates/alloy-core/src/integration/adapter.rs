//! Adapter trait and registry.
//!
//! This module defines the adapter interface that bridges protocol implementations
//! with the Alloy event system.
//!
//! # Capability-Based Initialization
//!
//! Adapters use a capability discovery pattern to find and use available transports:
//!
//! ```rust,ignore
//! impl Adapter for MyAdapter {
//!     async fn on_start(&self, ctx: &mut AdapterContext) -> AdapterResult<()> {
//!         // Get WebSocket server capability if available
//!         if let Some(ws_server) = ctx.transport().ws_server() {
//!             let handler = self.create_connection_handler();
//!             ws_server.listen("0.0.0.0:8080", "/ws", handler).await?;
//!         }
//!
//!         // Get WebSocket client capability if available
//!         if let Some(ws_client) = ctx.transport().ws_client() {
//!             let handler = self.create_connection_handler();
//!             let config = ClientConfig::default().with_token("secret");
//!             ws_client.connect("ws://127.0.0.1:9000/ws", handler, config).await?;
//!         }
//!
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots are managed dynamically:
//! - **Server transports**: New connections automatically become bots
//! - **Client transports**: Configured endpoints connect/reconnect automatically
//! - Bots can join/leave at any time during runtime

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::foundation::error::AdapterResult;
use crate::integration::capability::{
    BotManager, ConnectionHandle, ListenerHandle, TransportContext,
};

/// Context provided to adapters during initialization and runtime.
///
/// Provides access to:
/// - Transport capabilities for setting up connections
/// - Bot manager for tracking active bots
/// - Event dispatcher for dispatching events
pub struct AdapterContext {
    /// Available transport capabilities.
    transport: TransportContext,
    /// Bot manager for this adapter.
    bot_manager: Arc<BotManager>,
    /// Active listener handles (to keep them alive).
    listeners: Vec<ListenerHandle>,
    /// Active connection handles.
    connections: HashMap<String, ConnectionHandle>,
}

impl AdapterContext {
    /// Creates a new adapter context.
    pub fn new(transport: TransportContext, bot_manager: Arc<BotManager>) -> Self {
        Self {
            transport,
            bot_manager,
            listeners: Vec::new(),
            connections: HashMap::new(),
        }
    }

    /// Returns a reference to the transport context.
    pub fn transport(&self) -> &TransportContext {
        &self.transport
    }

    /// Returns a reference to the bot manager.
    pub fn bot_manager(&self) -> &Arc<BotManager> {
        &self.bot_manager
    }

    /// Registers a listener handle (keeps it alive).
    pub fn add_listener(&mut self, handle: ListenerHandle) {
        self.listeners.push(handle);
    }

    /// Registers a connection handle.
    pub fn add_connection(&mut self, handle: ConnectionHandle) {
        self.connections.insert(handle.id.clone(), handle);
    }

    /// Gets a connection handle by bot ID.
    pub fn get_connection(&self, bot_id: &str) -> Option<&ConnectionHandle> {
        self.connections.get(bot_id)
    }
}

/// The core adapter trait.
///
/// Adapters bridge protocol-specific implementations (like OneBot) with
/// the Alloy framework. They are responsible for:
///
/// - Discovering and using available transport capabilities
/// - Creating connection handlers for incoming/outgoing connections
/// - Parsing raw messages into events
/// - Managing protocol-specific logic
///
/// # Configuration-Based Creation
///
/// Each adapter defines its own configuration type and provides a method to create
/// instances from that configuration. The runtime handles all configuration loading.
///
/// ```rust,ignore
/// impl Adapter for OneBotAdapter {
///     fn adapter_name() -> &'static str { "onebot" }
///     
///     fn from_config_erased(config: Box<dyn Any>) -> AdapterResult<Arc<Self>> {
///         let config = config.downcast::<OneBotConfig>()
///             .map_err(|_| AdapterError::Internal("Invalid config type"))?;
///         Ok(Arc::new(Self { config: *config }))
///     }
/// }
/// ```
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Returns the adapter name (e.g., "onebot").
    ///
    /// This name is used to:
    /// - Identify the adapter in registry and logs
    /// - Locate the adapter's configuration in `alloy.yaml` (via `ConfigurableAdapter`)
    ///
    /// ```yaml
    /// adapters:
    ///   onebot:  # <- returned by Adapter::name()
    ///     connections: [...]
    /// ```
    fn name() -> &'static str
    where
        Self: Sized;

    /// Called when the adapter should start.
    ///
    /// The adapter should:
    /// 1. Query available transport capabilities from the context
    /// 2. Set up listeners/connections for the capabilities it needs
    /// 3. Register connection handlers
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn on_start(&self, ctx: &mut AdapterContext) -> AdapterResult<()> {
    ///     // Set up WebSocket server if available
    ///     if let Some(ws_server) = ctx.transport().ws_server() {
    ///         let handler = self.create_connection_handler();
    ///         let handle = ws_server.listen("0.0.0.0:8080", "/ws", handler).await?;
    ///         ctx.add_listener(handle);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn on_start(&self, ctx: &mut AdapterContext) -> AdapterResult<()>;

    /// Called when the adapter is shutting down.
    ///
    /// The adapter should clean up any resources. Listener and connection
    /// handles will be automatically dropped when the context is dropped.
    async fn on_shutdown(&self, _ctx: &mut AdapterContext) -> AdapterResult<()> {
        Ok(())
    }
}

/// A boxed adapter trait object.
pub type BoxedAdapter = Arc<dyn Adapter>;

/// Trait for adapters that can be created from configuration.
///
/// This is a separate trait to avoid the associated type problem with trait objects.
/// Adapters implement both `Adapter` and `ConfigurableAdapter`.
pub trait ConfigurableAdapter: Adapter {
    /// The configuration type for this adapter.
    type Config: serde::de::DeserializeOwned + Default;

    /// Creates an adapter from its configuration.
    ///
    /// The runtime deserializes the config from `alloy.yaml` and calls this method.
    fn from_config(config: Self::Config) -> AdapterResult<Arc<Self>>
    where
        Self: Sized;
}
