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
use tokio::sync::Mutex;
use tracing::{debug, info, trace, warn};

use crate::adapter::{Adapter, AdapterContext};
use crate::bot::BoxedBot;
use crate::error::AdapterResult;
use crate::event::{BoxedEvent, EventType};
use crate::message::RichText;
use crate::transport::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, TransportContext,
};

/// Event dispatcher — receives protocol events and distributes them to handlers.
///
/// Implementations must be async to allow spawning handler tasks and awaiting
/// their creation before returning.
///
/// Use `Arc<dyn Dispatcher>` to pass a dispatcher through the bridge layer.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// Dispatch `event` (originated from `bot`) to all registered handlers.
    ///
    /// Returns when the dispatch operation is complete (e.g., all handler tasks
    /// have been spawned, but not necessarily finished).
    async fn dispatch(&self, event: BoxedEvent, bot: BoxedBot);
}

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
    bots: Mutex<HashMap<String, BoxedBot>>,
    /// Event dispatcher — distributes parsed events to handlers.
    event_dispatcher: Arc<dyn Dispatcher>,
    /// Available transport capabilities.
    transport: TransportContext,
    /// Active listener handles (to keep them alive).
    listeners: Mutex<Vec<ListenerHandle>>,
    /// Active connection handles.
    connections: Mutex<HashMap<String, ConnectionHandle>>,
}

impl AdapterBridge {
    /// Creates a new adapter bridge.
    pub fn new(
        adapter: Arc<dyn Adapter>,
        event_dispatcher: Arc<dyn Dispatcher>,
        transport: TransportContext,
    ) -> Self {
        Self {
            adapter,
            bots: Mutex::new(HashMap::new()),
            event_dispatcher,
            transport,
            listeners: Mutex::new(Vec::new()),
            connections: Mutex::new(HashMap::new()),
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
        self.bots.lock().await.keys().cloned().collect()
    }

    /// Returns the count of active bots.
    pub async fn bot_count(&self) -> usize {
        self.bots.lock().await.len()
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
        let mut bots = self.bots.lock().await;
        if bots.contains_key(bot_id) {
            warn!(bot_id = %bot_id, "Bot already exists, not registering");
            return;
        }

        let bot = self.adapter.create_bot(bot_id, connection);
        bots.insert(bot_id.to_string(), bot);
        debug!(bot_id = %bot_id, "Bot registered");
    }

    async fn on_message(&self, bot_id: &str, data: &[u8]) {
        let Some(bot) = self.bots.lock().await.get(bot_id).cloned() else {
            return;
        };

        let Some(event) = self.adapter.parse_event(&bot, data).await else {
            return;
        };

        // Log at appropriate level
        if event.event_type() == EventType::Meta {
            trace!(bot_id = %bot_id, event = %event.event_name(), "Received meta event");
        } else {
            let text = event.get_rich_text();
            if !text.is_empty() {
                let text: RichText = text.into();
                info!(bot_id = %bot_id, event = %event.event_name(), text = %text, "Received message event");
            } else {
                info!(bot_id = %bot_id, event = %event.event_name(), "Received event");
            }
        }

        // Dispatch in a separate task so we don't block the transport receiver.
        let dispatcher = self.event_dispatcher.clone();
        tokio::spawn(async move {
            dispatcher.dispatch(event, bot).await;
        });
    }

    async fn on_disconnect(&self, bot_id: &str) {
        let bot = self.bots.lock().await.remove(bot_id);
        if let Some(bot) = bot {
            bot.on_disconnect().await;
            info!(bot_id = %bot_id, "Connection closed");
        }
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
        self.bridge.listeners.lock().await.push(handle);
    }

    async fn add_connection(&self, handle: ConnectionHandle) {
        self.bridge
            .connections
            .lock()
            .await
            .insert(handle.id.clone(), handle);
    }

    async fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        self.bridge.bots.lock().await.get(id).cloned()
    }

    fn as_connection_handler(&self) -> Arc<dyn ConnectionHandler> {
        self.bridge.clone() as Arc<dyn ConnectionHandler>
    }
}
