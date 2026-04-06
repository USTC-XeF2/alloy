//! Bot trait and related types.
//!
//! This module defines the `Bot` trait which represents an active bot instance
//! that can receive events and send messages.

use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{DowncastSync, impl_downcast};
use serde_json::Value;

use crate::error::ApiResult;
use crate::event::Scene;
use crate::message::Sendable;

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
pub trait Bot: DowncastSync + 'static {
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

    /// Sends a message in a given scene.
    ///
    /// # Arguments
    ///
    /// * `scene` - The scene to send the message to
    /// * `message` - The message content to send
    ///
    /// # Returns
    ///
    /// The message ID if successful.
    async fn send(&self, scene: &Scene, message: &dyn Sendable) -> ApiResult<String>;

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

impl_downcast!(sync Bot);

/// A boxed Bot trait object.
pub type BoxedBot = Arc<dyn Bot>;

/// Generates strongly-typed async API wrapper methods based on [`Bot::call_api`].
///
/// This macro is intended for adapter-specific bot implementations that expose
/// ergonomic API methods while sharing the same underlying request flow.
///
/// Each generated method:
/// 1. Uses the function name as the API action (`stringify!($name)`).
/// 2. Packs arguments into a JSON object.
/// 3. Calls `self.call_api(...)` and maps the response to the requested return shape.
///
/// # Supported Forms
///
/// 1. No return payload (`ApiResult<()>`):
///    `impl_api!(set_status, (online: bool));`
///
/// 2. Deserialize full response into a type:
///    `impl_api!(get_profile, (user_id: i64) -> UserProfile);`
///
/// 3. Deserialize a specific response field:
///    `impl_api!(get_msg_id, (seq: i64) -> String, "message_id");`
///
/// # Parameters
///
/// - Optional outer attributes can be attached to generated methods
///   (e.g. doc comments, cfg attributes).
/// - `$name`: generated function name, and also the action string.
/// - `($arg: $typ, ...)`: named parameters used both in signature and JSON body.
/// - `-> $ret`: target deserialize type for form (2) and (3).
/// - `$field`: JSON field key to extract before deserialization for form (3).
///
/// # Example
///
/// ```ignore
/// impl OneBotBot {
///     impl_api!(
///         /// Send a private message.
///         send_private_msg,
///         (user_id: i64, message: String) -> i64,
///         "message_id"
///     );
/// }
/// ```
#[macro_export]
macro_rules! impl_api {
    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ::alloy_core::error::ApiResult<()> {
            self.call_api(
                stringify!($name),
                ::serde_json::json!({ $(stringify!($arg): $arg),* })
            ).await?;
            Ok(())
        }
    };

    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) -> $ret:ty $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ::alloy_core::error::ApiResult<$ret> {
            let result = self.call_api(
                stringify!($name),
                ::serde_json::json!({ $(stringify!($arg): $arg),* })
            ).await?;
            Ok(::serde_json::from_value(result)?)
        }
    };

    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) -> $ret:ty, $field:expr $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ::alloy_core::error::ApiResult<$ret> {
            let result = self.call_api(
                stringify!($name),
                ::serde_json::json!({ $(stringify!($arg): $arg),* })
            ).await?;
            result
                .get($field)
                .cloned()
                .and_then(|v| ::serde_json::from_value::<$ret>(v).ok())
                .ok_or_else(|| ::alloy_core::error::ApiError::SerializationError(format!("Missing {}", $field)))
        }
    };
}
