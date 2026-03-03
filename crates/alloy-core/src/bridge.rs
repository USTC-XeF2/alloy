//! Adapter bridge implementation.
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//! Its methods are organized into three traits to clarify who may call what:
//!
//! | Trait | Caller | Methods |
//! |---|---|---|
//! | [`ConnectionHandler`](crate::transport::ConnectionHandler) | transport layer | `get_bot_id`, `register_connection`, `on_message`, `on_disconnect`, `add_listener` |
//! | [`AdapterContext`](crate::adapter::AdapterContext) | adapter implementation | `transport`, `get_bot`, `as_connection_handler` |
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
use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace, warn};

use crate::adapter::{Adapter, AdapterContext};
use crate::bot::BoxedBot;
use crate::error::AdapterResult;
use crate::event::{BoxedEvent, EventType};
use crate::message::RichText;
use crate::transport::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, Sender, TransportContext,
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
    /// Active bots and their connection handles, keyed by bot ID.
    entries: RwLock<HashMap<String, (BoxedBot, ConnectionHandle)>>,
    /// Event dispatcher — distributes parsed events to handlers.
    event_dispatcher: Arc<dyn Dispatcher>,
    /// Available transport capabilities.
    transport: TransportContext,
    /// Active listener handles (to keep them alive).
    listeners: Mutex<Vec<ListenerHandle>>,
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
            entries: RwLock::new(HashMap::new()),
            event_dispatcher,
            transport,
            listeners: Mutex::new(Vec::new()),
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
    pub fn bot_ids(&self) -> Vec<String> {
        self.entries.read().keys().cloned().collect()
    }

    /// Returns the count of active bots.
    pub fn bot_count(&self) -> usize {
        self.entries.read().len()
    }
}

// =============================================================================
// ConnectionHandler impl — called by transport layer
// =============================================================================

#[async_trait]
impl ConnectionHandler for AdapterBridge {
    fn get_bot_id(&self, conn_info: ConnectionInfo) -> crate::error::TransportResult<String> {
        self.adapter.get_bot_id(conn_info)
    }

    fn register_connection(&self, bot_id: &str, sender: Option<Sender>) -> CancellationToken {
        let mut entries = self.entries.write();
        if let Some((_, handle)) = entries.get_mut(bot_id) {
            // Bot already exists: upgrade sender if currently receive-only.
            if handle.sender.is_none() {
                if let Some(new_sender) = sender {
                    debug!(bot_id = %bot_id, "Bot upgraded to send-capable connection");
                    handle.sender = Some(new_sender);
                }
            } else {
                warn!(bot_id = %bot_id, "Bot already has send capability, keeping existing sender");
            }
            handle.shutdown_token.clone()
        } else {
            // Bot absent — create it fresh.
            let shutdown_token = CancellationToken::new();
            let handle = ConnectionHandle {
                sender,
                shutdown_token: shutdown_token.clone(),
            };
            let bot = self.adapter.create_bot(bot_id, handle.clone());
            entries.insert(bot_id.to_string(), (bot, handle));
            info!(bot_id = %bot_id, "Bot registered");
            shutdown_token
        }
    }

    async fn on_message(&self, bot_id: &str, data: &[u8]) {
        let Some((bot, _)) = self.entries.read().get(bot_id).cloned() else {
            return;
        };

        let Some(event) = self.adapter.on_message(&bot, data).await else {
            return;
        };

        // Log at appropriate level
        if event.event_type() == EventType::Meta {
            trace!(bot_id = %bot_id, event = %event.event_name(), "Received meta event");
        } else {
            let text = event.get_rich_text();
            if text.is_empty() {
                info!(bot_id = %bot_id, event = %event.event_name(), "Received event");
            } else {
                let text: RichText = text.into();
                info!(bot_id = %bot_id, event = %event.event_name(), text = %text, "Received message event");
            }
        }

        // Dispatch in a separate task so we don't block the transport receiver.
        let dispatcher = self.event_dispatcher.clone();
        tokio::spawn(async move {
            dispatcher.dispatch(event, bot).await;
        });
    }

    async fn on_disconnect(&self, bot_id: &str) {
        let entry = self.entries.write().remove(bot_id);
        if let Some((bot, _handle)) = entry {
            bot.on_disconnect().await;
            info!(bot_id = %bot_id, "Connection closed");
        }
    }

    fn add_listener(&self, handle: ListenerHandle) {
        self.listeners.lock().push(handle);
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

    fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        self.bridge
            .entries
            .read()
            .get(id)
            .map(|(bot, _)| bot.clone())
    }

    fn as_connection_handler(&self) -> Arc<dyn ConnectionHandler> {
        self.bridge.clone() as Arc<dyn ConnectionHandler>
    }
}
