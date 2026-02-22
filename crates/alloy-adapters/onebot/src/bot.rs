//! OneBot v11 Bot implementation.
//!
//! This module provides `OneBotBot`, a concrete implementation of the `Bot` trait
//! that provides strongly-typed API methods for all OneBot v11 APIs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotBot;
//! use alloy_core::{BoxedBot, EventArc, FromContext};
//!
//! async fn my_handler(bot: BoxedBot, event: EventArc<MessageEvent>) {
//!     // Downcast to OneBotBot for strongly-typed APIs
//!     if let Some(onebot) = bot.as_any().downcast_ref::<OneBotBot>() {
//!         // Send a private message
//!         onebot.send_private_msg(12345678, "Hello!", false).await.ok();
//!         
//!         // Or use the generic send method (passes event directly)
//!         bot.send(event.as_event(), "Reply!").await.ok();
//!     }
//! }
//! ```

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, trace};

use crate::model::api::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo, GroupMemberInfo, LoginInfo, Status,
    StrangerInfo, VersionInfo,
};
use crate::model::event::{GroupMessageEvent, PrivateMessageEvent};
use crate::model::message::OneBotMessage;
use crate::model::segment::Segment;
use alloy_core::{ApiError, ApiResult, Bot, ConnectionHandle, Event};

// =============================================================================
// OneBotBot
// =============================================================================

/// A OneBot v11 Bot implementation.
///
/// Provides strongly-typed API methods for all OneBot v11 APIs.
pub struct OneBotBot {
    /// Bot ID (self_id from events).
    id: String,
    /// Connection handle for sending messages.
    connection: ConnectionHandle,
    /// Pending API call responses.
    pending_calls: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Echo counter for generating unique echo IDs.
    echo_counter: AtomicU64,
    /// API call timeout duration.
    api_timeout: Duration,
}

impl OneBotBot {
    /// Creates a new OneBotBot.
    pub(crate) fn new(id: &str, connection: ConnectionHandle) -> Self {
        Self {
            id: id.to_string(),
            connection,
            pending_calls: Arc::new(Mutex::new(HashMap::new())),
            echo_counter: AtomicU64::new(1),
            api_timeout: Duration::from_secs(30),
        }
    }

    /// Handles an incoming response message.
    pub(crate) async fn handle_response(&self, response: &Value) {
        if let Some(echo) = response.get("echo").and_then(Value::as_u64) {
            let mut pending = self.pending_calls.lock().await;
            if let Some(tx) = pending.remove(&echo) {
                let _ = tx.send(response.clone());
            }
        }
    }

    /// Returns the number of pending API calls.
    ///
    /// This can be useful for monitoring or debugging purposes.
    pub async fn pending_call_count(&self) -> usize {
        self.pending_calls.lock().await.len()
    }
}

// =============================================================================
// Bot Trait Implementation
// =============================================================================

#[async_trait]
impl Bot for OneBotBot {
    fn id(&self) -> &str {
        &self.id
    }

    async fn call_api(&self, action: &str, params: Value) -> ApiResult<Value> {
        let echo = self.echo_counter.fetch_add(1, Ordering::SeqCst);

        // Create channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending call
        {
            let mut pending = self.pending_calls.lock().await;
            pending.insert(echo, tx);
        }

        // Build request
        let request = json!({
            "action": action,
            "params": params,
            "echo": echo
        });

        debug!(action = %action, echo = %echo, "Calling OneBot API");
        trace!(request = %request, "API request");

        // Send request
        let request_bytes = serde_json::to_vec(&request)?;

        self.connection.send(request_bytes).await?;

        // Wait for response with timeout
        match timeout(self.api_timeout, rx).await {
            Ok(Ok(response)) => {
                trace!(response = %response, "API response");

                // Check for API error
                if let Some(retcode) = response.get("retcode").and_then(Value::as_i64)
                    && retcode != 0
                {
                    let message = response
                        .get("message")
                        .or_else(|| response.get("wording"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error")
                        .to_string();
                    return Err(ApiError::ApiError { retcode, message });
                }

                // Return the data field if present, otherwise the whole response
                Ok(response.get("data").cloned().unwrap_or(response))
            }
            Ok(Err(_)) => {
                // Channel closed, probably shutdown
                Err(ApiError::NotConnected)
            }
            Err(_) => {
                // Timeout - remove pending call
                let mut pending = self.pending_calls.lock().await;
                pending.remove(&echo);
                Err(ApiError::Timeout)
            }
        }
    }

    async fn send(&self, event: &dyn Event, message: &str) -> ApiResult<String> {
        // Convert string message to OneBotMessage
        let onebot_msg: OneBotMessage = Segment::text(message).into();

        // Extract target: try downcast first, then fallback to raw_json
        let target_id = {
            // Try to downcast to GroupMessageEvent first
            if let Some(group_msg) = event.as_any().downcast_ref::<GroupMessageEvent>() {
                Some((true, group_msg.group_id))
            } else if let Some(private_msg) = event.as_any().downcast_ref::<PrivateMessageEvent>() {
                Some((false, private_msg.user_id))
            } else {
                // Fallback: parse raw_json
                if let Some(raw_json) = event.raw_json()
                    && let Ok(parsed) = serde_json::from_str::<Value>(raw_json)
                {
                    if let Some(group_id) = parsed.get("group_id").and_then(Value::as_i64) {
                        Some((true, group_id))
                    } else if let Some(user_id) = parsed.get("user_id").and_then(Value::as_i64) {
                        Some((false, user_id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        let (is_group, id) = target_id.ok_or(ApiError::MissingSession)?;

        // Send message with unified logic
        let message_id = if is_group {
            self.send_group_msg(id, onebot_msg).await?
        } else {
            self.send_private_msg(id, onebot_msg).await?
        };

        Ok(message_id.to_string())
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    async fn on_disconnect(&self) {
        let mut pending = self.pending_calls.lock().await;
        let count = pending.len();
        if count > 0 {
            debug!(
                count = count,
                "Clearing pending API calls due to disconnect"
            );
            pending.clear();
        }
    }
}

// =========================================================================
// Message APIs
// =========================================================================

macro_rules! impl_api {
    // No return value
    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ApiResult<()> {
            self.call_api(stringify!($name), json!({ $(stringify!($arg): $arg),* })).await?;
            Ok(())
        }
    };
    // Returns a type T (deserialized from "data" or full response)
    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) -> $ret:ty $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ApiResult<$ret> {
            let result = self.call_api(stringify!($name), json!({ $(stringify!($arg): $arg),* })).await?;
            Ok(serde_json::from_value::<$ret>(result)?)
        }
    };
    // Returns a specific field from the response
    ($(#[$meta:meta])* $name:ident, ($($arg:ident: $typ:ty),*) -> $ret:ty, $field:expr $(,)?) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $typ),*) -> ApiResult<$ret> {
            let result = self.call_api(stringify!($name), json!({ $(stringify!($arg): $arg),* })).await?;
            result
                .get($field)
                .cloned()
                .and_then(|v| serde_json::from_value::<$ret>(v).ok())
                .ok_or_else(|| ApiError::SerializationError(format!("Missing {}", $field)))
        }
    };
}

impl OneBotBot {
    impl_api!(
        /// Sends a private message.
        ///
        /// # Arguments
        /// * `user_id` - Target user's QQ number
        /// * `message` - Message content as OneBotMessage
        send_private_msg,
        (user_id: i64, message: OneBotMessage) -> i32,
        "message_id"
    );

    impl_api!(
        /// Sends a group message.
        ///
        /// # Arguments
        /// * `group_id` - Target group number
        /// * `message` - Message content as OneBotMessage
        send_group_msg,
        (group_id: i64, message: OneBotMessage) -> i32,
        "message_id"
    );

    /// Sends a message (auto-detect type based on parameters).
    ///
    /// If both `user_id` and `group_id` are provided, `message_type` determines which to use.
    pub async fn send_msg(
        &self,
        message_type: Option<&str>,
        user_id: Option<i64>,
        group_id: Option<i64>,
        message: OneBotMessage,
    ) -> ApiResult<i64> {
        let mut params = json!({
            "message": message
        });

        if let Some(mt) = message_type {
            params["message_type"] = json!(mt);
        }
        if let Some(uid) = user_id {
            params["user_id"] = json!(uid);
        }
        if let Some(gid) = group_id {
            params["group_id"] = json!(gid);
        }

        let result = self.call_api("send_msg", params).await?;

        result
            .get("message_id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ApiError::SerializationError("Missing message_id".into()))
    }

    impl_api!(
        /// Deletes (recalls) a message.
        delete_msg,
        (message_id: i32)
    );

    impl_api!(
        /// Gets a message by ID.
        get_msg,
        (message_id: i32) -> GetMsgResponse
    );

    impl_api!(
        /// Gets a forwarded message.
        get_forward_msg,
        (id: &str) -> OneBotMessage,
        "message"
    );

    impl_api!(
        /// Sends a like.
        send_like,
        (user_id: i64, times: u8)
    );

    // =========================================================================
    // Group Management APIs
    // =========================================================================

    impl_api!(
        /// Kicks a user from a group.
        set_group_kick,
        (group_id: i64, user_id: i64, reject_add_request: bool)
    );

    impl_api!(
        /// Bans a user in a group.
        ///
        /// # Arguments
        /// * `group_id` - Group number
        /// * `user_id` - User to ban
        /// * `duration` - Ban duration in seconds (0 = unban)
        set_group_ban,
        (group_id: i64, user_id: i64, duration: u32)
    );

    impl_api!(
        /// Bans an anonymous user in a group.
        set_group_anonymous_ban,
        (group_id: i64, anonymous_flag: &str, duration: u32)
    );

    impl_api!(
        /// Enables/disables whole group ban.
        set_group_whole_ban,
        (group_id: i64, enable: bool)
    );

    impl_api!(
        /// Sets/unsets a user as group admin.
        set_group_admin,
        (group_id: i64, user_id: i64, enable: bool)
    );

    impl_api!(
        /// Enables/disables anonymous chat in a group.
        set_group_anonymous,
        (group_id: i64, enable: bool)
    );

    impl_api!(
        /// Sets a user's group card (nickname).
        set_group_card,
        (group_id: i64, user_id: i64, card: &str)
    );

    impl_api!(
        /// Sets the group name.
        set_group_name,
        (group_id: i64, group_name: &str)
    );

    impl_api!(
        /// Leaves a group.
        set_group_leave,
        (group_id: i64, is_dismiss: bool)
    );

    impl_api!(
        /// Sets a user's special title in a group.
        set_group_special_title,
        (group_id: i64, user_id: i64, special_title: &str)
    );

    // =========================================================================
    // Friend/Group Request APIs
    // =========================================================================

    impl_api!(
        /// Handles a friend add request.
        set_friend_add_request,
        (flag: &str, approve: bool, remark: &str)
    );

    impl_api!(
        /// Handles a group add/invite request.
        set_group_add_request,
        (flag: &str, sub_type: &str, approve: bool, reason: &str)
    );

    // =========================================================================
    // Information APIs
    // =========================================================================

    impl_api!(
        /// Gets login info.
        get_login_info,
        () -> LoginInfo
    );

    impl_api!(
        /// Gets stranger info.
        get_stranger_info,
        (user_id: i64, no_cache: bool) -> StrangerInfo
    );

    impl_api!(
        /// Gets the friend list.
        get_friend_list,
        () -> Vec<FriendInfo>
    );

    impl_api!(
        /// Gets group info.
        get_group_info,
        (group_id: i64, no_cache: bool) -> GroupInfo
    );

    impl_api!(
        /// Gets the group list.
        get_group_list,
        () -> Vec<GroupInfo>
    );

    impl_api!(
        /// Gets group member info.
        get_group_member_info,
        (group_id: i64, user_id: i64, no_cache: bool) -> GroupMemberInfo
    );

    impl_api!(
        /// Gets the group member list.
        get_group_member_list,
        (group_id: i64) -> Vec<GroupMemberInfo>
    );

    /// Gets group honor info.
    pub async fn get_group_honor_info(&self, group_id: i64, honor_type: &str) -> ApiResult<Value> {
        self.call_api(
            "get_group_honor_info",
            json!({
                "group_id": group_id,
                "type": honor_type
            }),
        )
        .await
    }

    // =========================================================================
    // Credential APIs
    // =========================================================================

    impl_api!(
        /// Gets cookies for a domain.
        get_cookies,
        (domain: &str) -> String,
        "cookies"
    );

    impl_api!(
        /// Gets CSRF token.
        get_csrf_token,
        () -> i32,
        "token"
    );

    impl_api!(
        /// Gets credentials (cookies + CSRF token).
        get_credentials,
        (domain: &str) -> Credentials
    );

    // =========================================================================
    // File APIs
    // =========================================================================

    impl_api!(
        /// Gets a voice file.
        get_record,
        (file: &str, out_format: &str) -> String,
        "file"
    );

    impl_api!(
        /// Gets an image file.
        get_image,
        (file: &str) -> String,
        "file"
    );

    impl_api!(
        /// Checks if the bot can send images.
        can_send_image,
        () -> bool,
        "yes"
    );

    impl_api!(
        /// Checks if the bot can send voice.
        can_send_record,
        () -> bool,
        "yes"
    );

    // =========================================================================
    // System APIs
    // =========================================================================

    impl_api!(
        /// Gets the running status.
        get_status,
        () -> Status
    );

    impl_api!(
        /// Gets version info.
        get_version_info,
        () -> VersionInfo
    );

    impl_api!(
        /// Restarts the OneBot implementation.
        set_restart,
        (delay: u32)
    );

    impl_api!(
        /// Cleans the cache.
        clean_cache,
        ()
    );
}
