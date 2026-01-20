//! Bot trait and related types.
//!
//! This module defines the `Bot` trait which represents an active bot instance
//! that can receive events and send messages.

use async_trait::async_trait;
use serde_json::Value;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::foundation::event::Event;

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

/// Result type for API calls.
pub type ApiResult<T> = Result<T, ApiError>;

/// Error type for API calls.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// The bot is not connected.
    NotConnected,
    /// The API call timed out.
    Timeout,
    /// The API returned an error.
    ApiError { retcode: i32, message: String },
    /// Failed to serialize/deserialize.
    SerializationError(String),
    /// Transport error.
    TransportError(String),
    /// The event does not have the required session information.
    MissingSession(String),
    /// Other error.
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NotConnected => write!(f, "Bot is not connected"),
            ApiError::Timeout => write!(f, "API call timed out"),
            ApiError::ApiError { retcode, message } => {
                write!(f, "API error ({retcode}): {message}")
            }
            ApiError::SerializationError(e) => write!(f, "Serialization error: {e}"),
            ApiError::TransportError(e) => write!(f, "Transport error: {e}"),
            ApiError::MissingSession(e) => write!(f, "Missing session info: {e}"),
            ApiError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ApiError {}

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
