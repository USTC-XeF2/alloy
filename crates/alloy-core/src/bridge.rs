//! Adapter bridge implementation.
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//! Its methods are organized into three traits to clarify who may call what:
//!
//! | Trait | Caller | Methods |
//! |---|---|---|
//! | [`ConnectionHandler`](crate::transport::ConnectionHandler) | transport layer | `get_bot_id`, `register_connection`, `on_message`, `on_disconnect`, `add_listener` |
//! | [`Adapter`](crate::adapter::Adapter) lifecycle hooks | adapter implementation | `on_start`, `on_shutdown` |
//! | (direct methods) | runtime | `on_start`, `on_shutdown`, `bots` |
//!
//! # Architecture
//!
//! ```text
//! Transport ←→ Arc<dyn ConnectionHandler>
//!                     ↕ (implemented by AdapterBridge)
//! Adapter  ←→ (TransportContext, Arc<dyn ConnectionHandler>)
//!                     ↕
//! Runtime  ←→ Arc<AdapterBridge>
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace, warn};

use crate::adapter::Adapter;
use crate::bot::{Bot, BoxedBot};
use crate::error::AdapterResult;
use crate::event::{BoxedEvent, EventType};
use crate::message::RichText;
use crate::transport::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, Sender, TransportContext,
};

#[async_trait]
pub trait BridgeRuntime: Send + Sync {
    /// Starts the adapter (delegates to [`Adapter::on_start`]).
    async fn on_start(self: Arc<Self>) -> AdapterResult<()>;

    /// Shuts down the adapter (delegates to [`Adapter::on_shutdown`]).
    async fn on_shutdown(self: Arc<Self>) -> AdapterResult<()>;

    /// Returns a list of all active bot instances.
    fn bots(&self) -> Vec<Arc<dyn Bot>>;
}

/// Event dispatcher — receives protocol events and distributes them to handlers.
///
/// Implementations must be async to allow spawning handler tasks and awaiting
/// their creation before returning.
///
/// Use `Arc<dyn Dispatcher>` to pass a dispatcher through the bridge layer.
pub trait Dispatcher: Send + Sync + 'static {
    /// Dispatch `event` (originated from `bot`) to all registered handlers.
    ///
    /// Returns when the dispatch operation is complete (e.g., all handler tasks
    /// have been spawned, but not necessarily finished).
    fn dispatch(&self, event: BoxedEvent, bot: BoxedBot) -> impl Future<Output = ()> + Send;
}

// =============================================================================
// Adapter Bridge
// =============================================================================

/// Central bridge that wires together the runtime, the transport layer, and an adapter.
///
/// - Implements [`ConnectionHandler`] — transport implementations call it when
///   connections are established or data arrives.
/// - Exposes runtime-facing methods (`on_start`, `on_shutdown`, `bot_ids`, `bot_count`)
///   directly, since the runtime holds `Arc<AdapterBridge>`.
///
/// Each `AdapterBridge` manages bots for exactly one adapter instance.
#[derive(Debug)]
pub struct AdapterBridge<A: Adapter, D: Dispatcher> {
    adapter: A,
    /// Active bots and their connection handles, keyed by bot ID.
    entries: RwLock<HashMap<String, (Arc<A::Bot>, ConnectionHandle)>>,
    /// Event dispatcher — distributes parsed events to handlers.
    event_dispatcher: Arc<D>,
    /// Available transport capabilities.
    transport: TransportContext,
    /// Active listener handles (to keep them alive).
    listeners: Mutex<Vec<ListenerHandle>>,
}

impl<A: Adapter, D: Dispatcher> AdapterBridge<A, D> {
    /// Creates a new adapter bridge.
    pub fn new(adapter: A, event_dispatcher: Arc<D>, transport: TransportContext) -> Self {
        Self {
            adapter,
            entries: RwLock::new(HashMap::new()),
            event_dispatcher,
            transport,
            listeners: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl<A: Adapter, D: Dispatcher> BridgeRuntime for AdapterBridge<A, D> {
    /// Starts the adapter (delegates to [`Adapter::on_start`]).
    async fn on_start(self: Arc<Self>) -> AdapterResult<()> {
        self.adapter.on_start(self.transport, self.clone()).await
    }

    /// Shuts down the adapter (delegates to [`Adapter::on_shutdown`]).
    async fn on_shutdown(self: Arc<Self>) -> AdapterResult<()> {
        self.adapter.on_shutdown().await
    }

    /// Returns a list of all active bot instances.
    fn bots(&self) -> Vec<Arc<dyn Bot>> {
        self.entries
            .read()
            .values()
            .map(|(bot, _)| bot.clone() as Arc<dyn Bot>)
            .collect()
    }
}

// =============================================================================
// ConnectionHandler impl — called by transport layer
// =============================================================================

#[async_trait]
impl<A: Adapter, D: Dispatcher> ConnectionHandler for AdapterBridge<A, D> {
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
            let bot = self.adapter.create_bot(bot_id, &handle);
            entries.insert(bot_id.to_string(), (Arc::new(bot), handle));
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
