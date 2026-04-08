//! OneBot v11 Bot implementation.
//!
//! This module provides `OneBotBot`, a concrete implementation of the `Bot` trait
//! that provides strongly-typed API methods for all OneBot v11 APIs.
//!
//! # Overview
//!
//! Transport-specific strategies for OneBot v11 API calls:
//!
//! | Transport | Strategy |
//! |-----------|---------|
//! | WebSocket (server & client) | Async echo matching — request is tagged with a numeric echo; response arrives on the shared channel and is routed to the waiting future. |
//! | HTTP client | Synchronous POST — request body is sent as the HTTP body; the HTTP response body is the API response. No echo is needed. |
//! | Receive-only (`kind == None`) | Disabled — connections without a send capability cannot issue API calls. |
//!
//! # Usage
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotBot;
//! use alloy_core::{BoxedBot, EventArc, FromContext};
//!
//! async fn my_handler(bot: BoxedBot, event: EventArc<MessageEvent>) {
//!     // Downcast to OneBotBot for strongly-typed APIs
//!     if let Ok(onebot) = bot.clone().downcast_arc::<OneBotBot>() {
//!         // Send a private message
//!         onebot.send_private_msg(12345678, "Hello!", false).await.ok();
//!         
//!         // Or use the generic send method (passes event directly)
//!         bot.send(event.as_ref(), "Reply!").await.ok();
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::model::api::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo, GroupMemberInfo, LoginInfo, StrangerInfo,
    VersionInfo,
};
use crate::model::message::OneBotMessage;
use crate::model::types::Status;
use alloy_core::{
    ApiError, ApiResult, Bot, ConnectionHandle, PostJsonFn, Scene, Sendable, Sender,
    TransportError, impl_api,
};

// =============================================================================
// ApiCallStrategy
// =============================================================================

enum ApiCallStrategy {
    /// WebSocket caller with echo-based async routing
    Ws {
        message_tx: mpsc::Sender<Vec<u8>>,
        pending_calls: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
        echo_counter: AtomicU64,
        api_timeout: Duration,
    },
    /// HTTP client caller with POST function
    HttpClient { post_json: PostJsonFn },
    /// Disabled caller for receive-only connections
    Disabled,
}

impl ApiCallStrategy {
    /// Creates a new strategy from a connection handle.
    fn new(connection: &ConnectionHandle) -> Self {
        match connection.sender() {
            Some(Sender::HttpClient { post_json }) => Self::HttpClient {
                post_json: post_json.clone(),
            },
            Some(Sender::Ws { message_tx }) => Self::Ws {
                message_tx: message_tx.clone(),
                pending_calls: Mutex::new(HashMap::new()),
                echo_counter: AtomicU64::new(1),
                api_timeout: Duration::from_secs(30),
            },
            None => Self::Disabled,
        }
    }

    /// Makes an API call and returns the response data.
    async fn call(&self, action: &str, params: Value) -> ApiResult<Value> {
        match self {
            Self::Ws {
                message_tx,
                pending_calls,
                echo_counter,
                api_timeout,
            } => {
                let echo = echo_counter.fetch_add(1, Ordering::SeqCst);

                // Register pending response channel before sending so we never miss a
                // response that arrives before we start awaiting.
                let (tx, rx) = oneshot::channel();
                pending_calls.lock().insert(echo, tx);

                // Serialize and send the request.
                let request = json!({
                    "action": action,
                    "params": params,
                    "echo": echo
                });

                debug!(action = %action, echo = %echo, "Calling OneBot API via WebSocket");

                let request_bytes = serde_json::to_vec(&request)?;
                if let Err(e) = message_tx.send(request_bytes).await {
                    // Remove the pending entry so it doesn't dangle.
                    pending_calls.lock().remove(&echo);
                    return Err(TransportError::SendFailed(e.to_string()).into());
                }

                // Await the response with a timeout.
                match timeout(*api_timeout, rx).await {
                    Ok(Ok(response)) => Ok(response),
                    Ok(Err(_)) => {
                        // Channel closed — transport was shut down.
                        Err(ApiError::NotConnected)
                    }
                    Err(_) => {
                        // Timed out — remove the pending entry.
                        pending_calls.lock().remove(&echo);
                        Err(ApiError::Timeout)
                    }
                }
            }
            Self::HttpClient { post_json } => {
                let body = json!({
                    "action": action,
                    "params": params,
                });

                debug!(action = %action, "Calling OneBot API via HTTP");

                let response_json = (post_json)("", body)
                    .await
                    .map_err(|e| ApiError::Other(format!("HTTP request failed: {e}")))?;

                Ok(response_json)
            }
            Self::Disabled => Err(ApiError::NotSupported),
        }
    }

    /// Routes an incoming protocol message that is an API response.
    fn on_incoming_response(&self, data: &Value) {
        if let Self::Ws { pending_calls, .. } = self
            && let Some(echo) = data.get("echo").and_then(Value::as_u64)
        {
            let mut pending = pending_calls.lock();
            if let Some(tx) = pending.remove(&echo) {
                let _ = tx.send(data.clone());
            } else {
                // Echo arrived but no waiter — was probably already timed out.
                warn!(echo = %echo, "Received WS API response for unknown echo (timed out?)");
            }
        }
    }

    /// Called when the underlying transport connection is closed.
    fn on_disconnect(&self) {
        if let Self::Ws { pending_calls, .. } = self {
            let mut pending = pending_calls.lock();
            let count = pending.len();
            if count > 0 {
                debug!(
                    count = count,
                    "Clearing pending WS API calls due to disconnect"
                );
                pending.clear();
            }
        }
    }
}

// =============================================================================
// OneBotBot
// =============================================================================

/// A OneBot v11 Bot implementation.
///
/// Wraps an [`ApiCallStrategy`] that handles the transport-specific request/response
/// strategy (WebSocket echo-matching or direct HTTP POST).
pub struct OneBotBot {
    /// Bot ID (self_id from events).
    id: String,
    /// Transport-specific API call mechanism (enum-based, no dyn dispatch).
    call_strategy: ApiCallStrategy,
}

impl OneBotBot {
    /// Creates a new `OneBotBot` from a connection handle.
    pub(crate) fn new(id: &str, connection: &ConnectionHandle) -> Self {
        Self {
            id: id.into(),
            call_strategy: ApiCallStrategy::new(connection),
        }
    }

    pub(crate) fn handle_response(&self, data: &Value) {
        self.call_strategy.on_incoming_response(data);
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
        let mut response = self.call_strategy.call(action, params).await?;
        let response = response.as_object_mut().ok_or_else(|| {
            ApiError::SerializationError("Expected API response to be an object".into())
        })?;

        if let Some(retcode) = response.get("retcode").and_then(Value::as_i64)
            && retcode != 0
        {
            let message = response
                .get("message")
                .or_else(|| response.get("wording"))
                .and_then(Value::as_str)
                .unwrap_or("Unknown error")
                .to_string();
            return Err(ApiError::ApiError { retcode, message });
        }

        response.remove("data").ok_or_else(|| {
            ApiError::SerializationError("Expected API response to contain data".into())
        })
    }

    async fn send(&self, scene: &Scene, message: &dyn Sendable) -> ApiResult<String> {
        let onebot_msg = OneBotMessage::from_erased_message(message);

        match scene {
            Scene::Group { group_id, .. } => {
                if let Ok(group_id) = group_id.parse::<i64>() {
                    Some(self.send_group_msg(group_id, &onebot_msg).await?)
                } else {
                    None
                }
            }
            Scene::Private { user_id } => {
                if let Ok(user_id) = user_id.parse::<i64>() {
                    Some(self.send_private_msg(user_id, &onebot_msg).await?)
                } else {
                    None
                }
            }
            _ => None,
        }
        .map(|id| id.to_string())
        .ok_or_else(|| ApiError::Other("unsupported scene for OneBotBot".into()))
    }

    async fn on_disconnect(&self) {
        self.call_strategy.on_disconnect();
    }
}

impl OneBotBot {
    // =========================================================================
    // Message APIs
    // =========================================================================

    impl_api!(
        /// Sends a private message.
        ///
        /// # Arguments
        /// * `user_id` - Target user's QQ number
        /// * `message` - Message content as OneBotMessage
        send_private_msg,
        (user_id: i64, message: &OneBotMessage) -> i32,
        "message_id"
    );

    impl_api!(
        /// Sends a group message.
        ///
        /// # Arguments
        /// * `group_id` - Target group number
        /// * `message` - Message content as OneBotMessage
        send_group_msg,
        (group_id: i64, message: &OneBotMessage) -> i32,
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
        message: &OneBotMessage,
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
