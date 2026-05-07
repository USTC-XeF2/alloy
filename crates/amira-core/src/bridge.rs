//! Adapter bridge implementation.
//!
//! The [`AdapterBridge`] sits between the transport layer and the adapter,
//! handling common bot lifecycle (registration, event dispatch, cleanup) automatically.
//! Its methods are organized into three traits to clarify who may call what:
//!
//! | Trait | Caller | Methods |
//! |---|---|---|
//! | [`ConnectionHandler`](crate::transport::ConnectionHandler) | transport layer | `register_connection`, `on_message`, `on_disconnect`, `add_listener` |
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
use bytes::Bytes;
use parking_lot::Mutex;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, trace, warn};

use crate::adapter::Adapter;
use crate::bot::Bot;
use crate::error::AdapterResult;
use crate::event::{EventRoot, EventType, PlatformEvent};
use crate::transport::{ConnectionHandler, ListenerHandle, Sender, TransportContext};

#[async_trait]
pub trait BridgeRuntime: Send + Sync {
    /// Starts the adapter (delegates to [`Adapter::on_start`]).
    async fn start(self: Arc<Self>) -> AdapterResult<()>;

    /// Shuts down the adapter.
    async fn shutdown(&self);

    /// Waits until all bridge-owned listener handles finish.
    async fn wait(&self);

    /// Returns a list of all active bot instances.
    fn bots(&self) -> Vec<Arc<dyn Bot>>;
}

/// Event dispatcher — receives protocol events and distributes them to handlers.
///
/// Implementations must be async to allow spawning handler tasks and awaiting
/// their creation before returning.
pub trait Dispatcher: Send + Sync + 'static {
    /// Dispatch `event` (originated from `bot`) to all registered handlers.
    ///
    /// Returns when the dispatch operation is complete (e.g., all handler tasks
    /// have been spawned, but not necessarily finished).
    fn dispatch<E, B>(&self, event: E, bot: Arc<B>) -> impl Future<Output = ()> + Send
    where
        E: EventRoot,
        B: Bot;
}

type BotEntry<B> = (Arc<B>, watch::Receiver<()>, watch::Sender<()>);

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
    entries: Mutex<HashMap<String, BotEntry<A::Bot>>>,
    /// Event dispatcher — distributes parsed events to handlers.
    event_dispatcher: Arc<D>,
    /// Available transport capabilities.
    transport: TransportContext,
    /// Active listener handles (to keep them alive).
    listeners: Mutex<Vec<ListenerHandle>>,
    /// Active background tasks (to keep them alive).
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl<A: Adapter, D: Dispatcher> AdapterBridge<A, D> {
    /// Creates a new adapter bridge.
    pub fn new(adapter: A, event_dispatcher: Arc<D>, transport: TransportContext) -> Self {
        Self {
            adapter,
            entries: Mutex::default(),
            event_dispatcher,
            transport,
            listeners: Mutex::default(),
            tasks: Mutex::default(),
        }
    }
}

#[async_trait]
impl<A: Adapter, D: Dispatcher> BridgeRuntime for AdapterBridge<A, D> {
    /// Starts the adapter (delegates to [`Adapter::on_start`]).
    async fn start(self: Arc<Self>) -> AdapterResult<()> {
        self.adapter.on_start(self.transport, self.clone()).await
    }

    /// Shuts down the adapter.
    async fn shutdown(&self) {
        // Stop accepting new connections before tearing down active ones.
        let listeners = {
            let mut listeners = self.listeners.lock();
            std::mem::take(&mut *listeners)
        };
        drop(listeners);

        // Cancel all active connection loops and notify bots.
        let entries = {
            let mut entries = self.entries.lock();
            std::mem::take(&mut *entries)
        };

        for (_, (bot, _rx, _)) in entries {
            bot.on_disconnect().await;
        }

        self.wait().await;
    }

    async fn wait(&self) {
        let tasks = {
            let mut tasks = self.tasks.lock();
            std::mem::take(&mut *tasks)
        };

        for task in tasks {
            if let Err(e) = task.await {
                warn!(error = %e, "Background task terminated with join error");
            }
        }
    }

    /// Returns a list of all active bot instances.
    fn bots(&self) -> Vec<Arc<dyn Bot>> {
        self.entries
            .lock()
            .values()
            .map(|(bot, _, _)| bot.clone() as Arc<dyn Bot>)
            .collect()
    }
}

#[async_trait]
impl<A: Adapter, D: Dispatcher> ConnectionHandler for AdapterBridge<A, D> {
    fn register_connection(&self, bot_id: &str, sender: Option<Sender>) -> watch::Sender<()> {
        let mut entries = self.entries.lock();
        if let Some((_, _, shutdown_tx)) = entries.get_mut(bot_id) {
            if sender.is_some() {
                warn!(bot_id = %bot_id, "Bot already registered, ignoring new sender capability");
            }
            shutdown_tx.clone()
        } else {
            // Bot absent — create it fresh.
            let (shutdown_tx, shutdown_rx) = watch::channel(());
            let bot = self.adapter.create_bot(bot_id, sender);
            entries.insert(
                bot_id.to_string(),
                (Arc::new(bot), shutdown_rx, shutdown_tx.clone()),
            );
            info!(bot_id = %bot_id, "Bot registered");
            shutdown_tx
        }
    }

    async fn on_message(&self, bot_id: &str, data: Bytes) {
        let Some((bot, _, _)) = self.entries.lock().get(bot_id).cloned() else {
            return;
        };

        let Some(event) = self.adapter.on_message(&bot, data).await else {
            return;
        };

        // Log at appropriate level
        match event.event_type() {
            EventType::Message => info!(
                bot_id = %bot_id,
                platform = %A::Event::PLATFORM,
                event_id = %event.event_id(),
                text = %event.rich_text(),
                "Received message event"
            ),
            EventType::Meta => trace!(
                bot_id = %bot_id,
                platform = %A::Event::PLATFORM,
                event_id = %event.event_id(),
                "Received meta event"
            ),
            _ => info!(
                bot_id = %bot_id,
                platform = %A::Event::PLATFORM,
                event_id = %event.event_id(),
                "Received event"
            ),
        }

        // Dispatch in a separate task so we don't block the transport receiver.
        let dispatcher = self.event_dispatcher.clone();
        tokio::spawn(async move {
            dispatcher.dispatch(event, bot).await;
        });
    }

    async fn on_disconnect(&self, bot_id: &str) {
        let entry = self.entries.lock().remove(bot_id);
        if let Some((bot, _rx, _)) = entry {
            bot.on_disconnect().await;
            info!(bot_id = %bot_id, "Connection closed");
        }
    }

    fn add_listener(&self, handle: ListenerHandle, task: JoinHandle<()>) {
        self.listeners.lock().push(handle);
        self.tasks.lock().push(task);
    }

    fn add_task(&self, task: JoinHandle<()>) {
        self.tasks.lock().push(task);
    }
}
