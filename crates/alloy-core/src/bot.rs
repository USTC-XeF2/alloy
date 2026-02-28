//! Bot trait and related types.
//!
//! This module defines the `Bot` trait which represents an active bot instance
//! that can receive events and send messages.

use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ApiResult;
use crate::event::Event;
use crate::message::ErasedMessage;

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
pub trait Bot: Send + Sync + 'static {
    /// Returns the bot's unique identifier.
    fn id(&self) -> &str;

    /// Calls a raw API with the given action name and parameters.
    ///
    /// This is the low-level API that all other methods should use.
    ///
    /// # Arguments
    ///
    /// * `action` - The API action name (e.g., "send_private_msg")
    /// * `params` - JSON value containing the parameters
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API.
    async fn call_api(&self, action: &str, params: Value) -> ApiResult<Value>;

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
    async fn send(&self, event: &dyn Event, message: &str) -> ApiResult<String>;

    /// Sends a rich (type-erased) message in response to an event.
    ///
    /// # Arguments
    ///
    /// * `event`   - The event to respond to
    /// * `message` - A type-erased [`ErasedMessage`]; pass any `Message<S>` reference
    async fn send_message(
        &self,
        event: &dyn Event,
        message: &dyn ErasedMessage,
    ) -> ApiResult<String>;

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

    /// Called when the transport connection is lost.
    ///
    /// Implementations should clean up any pending state, such as:
    /// - Pending API call responses (notify waiters of disconnection)
    /// - Cached session data
    /// - Protocol-specific cleanup
    ///
    /// This is called by [`AdapterBridge`](crate::adapter::AdapterBridge)
    /// before the bot is unregistered from the [`BotManager`].
    ///
    /// The default implementation does nothing.
    async fn on_disconnect(&self) {}
}

/// A boxed Bot trait object.
pub type BoxedBot = Arc<dyn Bot>;
