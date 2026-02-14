//! Bot trait and related types.
//!
//! This module defines the `Bot` trait which represents an active bot instance
//! that can receive events and send messages.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

use crate::error::{ApiResult, TransportError, TransportResult};
use crate::event::{BoxedEvent, Event};
use crate::transport::ConnectionHandle;

/// Message sent from Bot to Runtime.
#[derive(Debug)]
pub enum BotMessage {
    /// Send raw data through the transport.
    Send(Vec<u8>),
    /// Send a text message through the transport.
    SendText(String),
    /// Request to shutdown this bot.
    Shutdown,
    /// Log a message at info level.
    Log(String),
}

/// Message sent from Runtime to Bot.
#[derive(Debug)]
pub enum RuntimeMessage {
    /// Raw data received from transport.
    Received(Vec<u8>),
    /// Transport connected.
    Connected,
    /// Transport disconnected.
    Disconnected(Option<String>),
    /// Shutdown request.
    Shutdown,
}

/// Bot communication channels.
pub struct BotChannels {
    /// Sender for messages to runtime.
    pub to_runtime: mpsc::Sender<BotMessage>,
    /// Receiver for messages from runtime.
    pub from_runtime: mpsc::Receiver<RuntimeMessage>,
}

/// Runtime side of bot communication channels.
pub struct RuntimeChannels {
    /// Sender for messages to bot.
    pub to_bot: mpsc::Sender<RuntimeMessage>,
    /// Receiver for messages from bot.
    pub from_bot: mpsc::Receiver<BotMessage>,
}

/// Creates a pair of communication channels between bot and runtime.
pub fn create_bot_channels(buffer_size: usize) -> (BotChannels, RuntimeChannels) {
    let (to_runtime_tx, to_runtime_rx) = mpsc::channel(buffer_size);
    let (to_bot_tx, to_bot_rx) = mpsc::channel(buffer_size);

    let bot_channels = BotChannels {
        to_runtime: to_runtime_tx,
        from_runtime: to_bot_rx,
    };

    let runtime_channels = RuntimeChannels {
        to_bot: to_bot_tx,
        from_bot: to_runtime_rx,
    };

    (bot_channels, runtime_channels)
}

/// The core Bot trait.
///
/// A Bot is an active instance that:
/// - Receives events from the runtime
/// - Processes events through handlers
/// - Sends messages back through the transport
///
/// Each bot instance is associated with an adapter that defines
/// how protocol-specific messages are parsed and serialized.
///
/// # API Design
///
/// - `call_api`: Raw API call with action name and JSON parameters
/// - `send`: Unified message sending that extracts session from event
///
/// Concrete implementations (e.g., `OneBotBot`) should provide
/// strongly-typed API methods on top of `call_api`.
#[async_trait]
pub trait Bot: Send + Sync {
    /// Returns the bot's unique identifier.
    fn id(&self) -> &str;

    /// Returns the adapter name this bot uses.
    fn adapter_name(&self) -> &str;

    /// Calls a raw API with the given action name and parameters.
    ///
    /// This is the low-level API that all other methods should use.
    ///
    /// # Arguments
    ///
    /// * `action` - The API action name (e.g., "send_private_msg")
    /// * `params` - JSON string containing the parameters
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API.
    async fn call_api(&self, action: &str, params: &str) -> ApiResult<Value>;

    /// Sends a message in response to an event.
    ///
    /// This method extracts the session information (user_id, group_id, etc.)
    /// from the event and constructs the appropriate API call.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to respond to
    /// * `message` - The message content to send
    ///
    /// # Returns
    ///
    /// The message ID if successful.
    async fn send(&self, event: &dyn Event, message: &str) -> ApiResult<i64>;

    /// Returns self as an `Arc<dyn Any>` for safe downcasting.
    ///
    /// This method takes `Arc<Self>` to enable safe downcasting to concrete types
    /// using `Arc::downcast`. Implementors should simply return `self`.
    ///
    /// # Example Implementation
    ///
    /// ```rust,ignore
    /// fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
    ///     self
    /// }
    /// ```
    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

/// A boxed Bot trait object.
pub type BoxedBot = Arc<dyn Bot>;

/// Attempts to downcast a BoxedBot to a specific concrete type.
///
/// This is used by the extractor system to allow handlers to receive
/// concrete bot types like `Arc<OneBotBot>` instead of just `BoxedBot`.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::{BoxedBot, downcast_bot};
/// use alloy_adapter_onebot::OneBotBot;
///
/// let boxed: BoxedBot = /* ... */;
/// if let Some(onebot) = downcast_bot::<OneBotBot>(boxed) {
///     // Use protocol-specific APIs
///     onebot.send_private_msg(12345, "Hello!", false).await.ok();
/// }
/// ```
pub fn downcast_bot<T: Bot + 'static>(bot: BoxedBot) -> Option<Arc<T>> {
    let any_arc = bot.as_any();
    Arc::downcast::<T>(any_arc).ok()
}

// =============================================================================
// Bot Manager
// =============================================================================

/// Manages dynamically connected bots.
///
/// Bots can join and leave at any time:
/// - Server: new connections add bots, disconnections remove them
/// - Client: configured endpoints create bots, with auto-reconnect on disconnect
pub struct BotManager {
    /// Active bots by ID.
    bots: RwLock<HashMap<String, BotEntry>>,
    /// Event dispatcher callback (always requires bot).
    event_dispatcher: Arc<dyn Fn(BoxedEvent, BoxedBot) + Send + Sync>,
}

/// Entry for a managed bot.
struct BotEntry {
    /// Bot ID.
    id: String,
    /// Connection handle for sending messages.
    connection: ConnectionHandle,
    /// Adapter name that owns this bot.
    adapter: String,
    /// Bot metadata.
    metadata: HashMap<String, String>,
    /// Bot instance (if available).
    bot: BoxedBot,
}

impl std::fmt::Debug for BotEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotEntry")
            .field("id", &self.id)
            .field("connection", &self.connection)
            .field("adapter", &self.adapter)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl BotManager {
    /// Creates a new bot manager with the event dispatcher.
    ///
    /// The dispatcher callback receives both the event and the associated bot.
    pub fn new(event_dispatcher: Arc<dyn Fn(BoxedEvent, BoxedBot) + Send + Sync>) -> Self {
        Self {
            bots: RwLock::new(HashMap::new()),
            event_dispatcher,
        }
    }

    /// Registers a new bot with a Bot instance.
    pub async fn register(
        &self,
        id: String,
        connection: ConnectionHandle,
        adapter: String,
        bot: BoxedBot,
    ) -> TransportResult<()> {
        let mut bots = self.bots.write().await;
        if bots.contains_key(&id) {
            return Err(TransportError::BotAlreadyExists { id });
        }
        bots.insert(
            id.clone(),
            BotEntry {
                id,
                connection,
                adapter,
                metadata: HashMap::new(),
                bot,
            },
        );
        Ok(())
    }

    /// Gets the bot instance by ID.
    pub async fn get_bot(&self, id: &str) -> Option<BoxedBot> {
        let bots = self.bots.read().await;
        bots.get(id).map(|e| e.bot.clone())
    }

    /// Unregisters a bot.
    pub async fn unregister(&self, id: &str) -> Option<ConnectionHandle> {
        let mut bots = self.bots.write().await;
        bots.remove(id).map(|e| e.connection)
    }

    /// Gets a connection handle for a bot.
    pub async fn get_connection(&self, id: &str) -> Option<ConnectionHandle> {
        let bots = self.bots.read().await;
        bots.get(id).map(|e| e.connection.clone())
    }

    /// Dispatches an event with the associated bot.
    ///
    /// If the bot instance is registered, this will dispatch the event with that bot.
    ///
    /// # Arguments
    ///
    /// * `bot_id` - The ID of the bot associated with this event
    /// * `event` - The event to dispatch
    ///
    /// # Returns
    ///
    /// `true` if the event was dispatched, `false` if the bot was not found.
    pub async fn dispatch_event(&self, bot_id: &str, event: BoxedEvent) -> bool {
        if let Some(bot) = self.get_bot(bot_id).await {
            (self.event_dispatcher)(event, bot);
            return true;
        }
        tracing::warn!(bot_id = %bot_id, "Cannot dispatch event: bot not found");
        false
    }

    /// Returns the IDs of all active bots.
    pub async fn bot_ids(&self) -> Vec<String> {
        let bots = self.bots.read().await;
        bots.keys().cloned().collect()
    }

    /// Returns the count of active bots.
    pub async fn bot_count(&self) -> usize {
        let bots = self.bots.read().await;
        bots.len()
    }

    /// Sends a message to a specific bot.
    pub async fn send_to(&self, bot_id: &str, data: Vec<u8>) -> TransportResult<()> {
        let bots = self.bots.read().await;
        let bot = bots
            .get(bot_id)
            .ok_or_else(|| TransportError::BotNotFound {
                id: bot_id.to_string(),
            })?;
        bot.connection.send(data).await
    }

    /// Broadcasts a message to all bots.
    pub async fn broadcast(&self, data: Vec<u8>) -> Vec<TransportResult<()>> {
        let bots = self.bots.read().await;
        let mut results = Vec::new();
        for bot in bots.values() {
            results.push(bot.connection.send(data.clone()).await);
        }
        results
    }
}
