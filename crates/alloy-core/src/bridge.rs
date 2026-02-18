//! Adapter bridge implementation.
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//! Its methods are organized into three traits to clarify who may call what:
//!
//! | Trait | Caller | Methods |
//! |---|---|---|
//! | [`ConnectionHandler`](crate::transport::ConnectionHandler) | transport layer | `get_bot_id`, `create_bot`, `on_message`, `on_disconnect`, `on_error` |
//! | [`AdapterContext`](crate::adapter::AdapterContext) | adapter implementation | `transport`, `add_listener`, `add_connection`, `get_bot` |
//! | (direct methods) | runtime | `on_start`, `on_shutdown`, `bot_ids`, `bot_count` |
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

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::adapter::{Adapter, AdapterContext, Dispatcher};
use crate::bot::BoxedBot;
use crate::error::AdapterResult;
use crate::event::BoxedEvent;
use crate::transport::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, TransportContext,
};

// =============================================================================
// Adapter Bridge
// =============================================================================

/// Central bridge that wires together the runtime, the transport layer, and an adapter.
///
/// - Implements [`ConnectionHandler`] — transport implementations call it when
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
        let ctx: Arc<dyn AdapterContext> = Arc::new(AdapterContextWrapper {
            bridge: self.clone(),
        });
        self.adapter.on_start(ctx).await
    }

    /// Shuts down the adapter (delegates to [`Adapter::on_shutdown`]).
    pub async fn on_shutdown(self: &Arc<Self>) -> AdapterResult<()> {
        let ctx: Arc<dyn AdapterContext> = Arc::new(AdapterContextWrapper {
            bridge: self.clone(),
        });
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
    async fn get_bot_id(&self, conn_info: ConnectionInfo) -> crate::error::TransportResult<String> {
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

/// Wrapper type that carries an Arc<AdapterBridge> and implements AdapterContext.
/// This allows as_connection_handler() to return Arc<dyn ConnectionHandler>.
struct AdapterContextWrapper {
    bridge: Arc<AdapterBridge>,
}

#[async_trait]
impl AdapterContext for AdapterContextWrapper {
    fn transport(&self) -> &TransportContext {
        &self.bridge.transport
    }

    async fn add_listener(&self, handle: ListenerHandle) {
        self.bridge.listeners.write().await.push(handle);
    }

    async fn add_connection(&self, handle: ConnectionHandle) {
        self.bridge
            .connections
            .write()
            .await
            .insert(handle.id.clone(), handle);
    }

    async fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        self.bridge.bots.read().await.get(id).cloned()
    }

    fn as_connection_handler(&self) -> Arc<dyn ConnectionHandler> {
        self.bridge.clone() as Arc<dyn ConnectionHandler>
    }
}
