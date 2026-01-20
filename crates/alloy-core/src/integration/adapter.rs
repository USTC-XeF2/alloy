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
//!     async fn on_init(&self, ctx: &mut AdapterContext) -> anyhow::Result<()> {
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

use crate::foundation::event::BoxedEvent;
use crate::integration::capability::{
    BotManager, BoxedConnectionHandler, ConnectionHandle, ListenerHandle, TransportContext,
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
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Returns the adapter name (e.g., "onebot").
    fn name(&self) -> &'static str;

    /// Called when the adapter is initialized.
    ///
    /// The adapter should prepare internal state but not start connections yet.
    async fn on_init(&self, ctx: &mut AdapterContext) -> anyhow::Result<()> {
        let _ = ctx;
        Ok(())
    }

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
    /// async fn on_start(&self, ctx: &mut AdapterContext) -> anyhow::Result<()> {
    ///     // Set up WebSocket server if available
    ///     if let Some(ws_server) = ctx.transport().ws_server() {
    ///         let handler = self.create_connection_handler();
    ///         let handle = ws_server.listen("0.0.0.0:8080", "/ws", handler).await?;
    ///         ctx.add_listener(handle);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn on_start(&self, ctx: &mut AdapterContext) -> anyhow::Result<()>;

    /// Called when the adapter is shutting down.
    ///
    /// The adapter should clean up any resources. Listener and connection
    /// handles will be automatically dropped when the context is dropped.
    async fn on_shutdown(&self, _ctx: &mut AdapterContext) -> anyhow::Result<()> {
        Ok(())
    }

    /// Creates a connection handler for this adapter.
    ///
    /// The handler will be called for connection lifecycle events.
    fn create_connection_handler(&self) -> BoxedConnectionHandler;

    /// Parses raw data into an event.
    ///
    /// Returns `Ok(None)` if the data is not an event (e.g., API response).
    fn parse_event(&self, data: &[u8]) -> anyhow::Result<Option<BoxedEvent>>;

    /// Clones this adapter into an Arc.
    fn clone_adapter(&self) -> Arc<dyn Adapter>;
}

/// A boxed adapter trait object.
pub type BoxedAdapter = Arc<dyn Adapter>;

/// Factory for creating adapters.
///
/// Each adapter implementation provides a factory that can create
/// instances with the appropriate configuration.
pub trait AdapterFactory: Send + Sync {
    /// Returns the adapter name.
    fn name(&self) -> &'static str;

    /// Creates a new adapter instance.
    fn create(&self) -> BoxedAdapter;
}

/// Registry of available adapters.
pub struct AdapterRegistry {
    factories: HashMap<String, Box<dyn AdapterFactory>>,
}

impl AdapterRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Registers an adapter factory.
    pub fn register<F: AdapterFactory + 'static>(&mut self, factory: F) {
        self.factories
            .insert(factory.name().to_string(), Box::new(factory));
    }

    /// Gets a factory by adapter name.
    pub fn get(&self, name: &str) -> Option<&dyn AdapterFactory> {
        self.factories.get(name).map(AsRef::as_ref)
    }

    /// Creates an adapter by name.
    pub fn create(&self, name: &str) -> anyhow::Result<BoxedAdapter> {
        let factory = self
            .factories
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown adapter: {name}"))?;
        Ok(factory.create())
    }

    /// Returns the names of all registered adapters.
    pub fn adapters(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Legacy types (for backward compatibility during migration)
// =============================================================================

/// Adapter capabilities declaration (legacy).
///
/// This type is kept for backward compatibility but is deprecated
/// in favor of the capability discovery pattern.
#[deprecated(
    since = "0.2.0",
    note = "Use capability discovery pattern with TransportContext instead"
)]
#[derive(Debug, Clone)]
pub struct AdapterCapabilities {
    /// List of supported transport types.
    pub supported_transports: Vec<crate::integration::transport::TransportType>,
    /// The recommended/default transport type.
    pub recommended_transport: crate::integration::transport::TransportType,
}

#[allow(deprecated)]
impl AdapterCapabilities {
    /// Creates new capabilities.
    pub fn new(
        supported: Vec<crate::integration::transport::TransportType>,
        recommended: crate::integration::transport::TransportType,
    ) -> Self {
        Self {
            supported_transports: supported,
            recommended_transport: recommended,
        }
    }

    /// Checks if a transport type is supported.
    pub fn supports(&self, transport_type: crate::integration::transport::TransportType) -> bool {
        self.supported_transports.contains(&transport_type)
    }
}
