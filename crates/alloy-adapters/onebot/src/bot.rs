//! OneBot v11 Bot implementation.
//!
//! This module provides `OneBotBot`, a concrete implementation of the `Bot` trait
//! that provides strongly-typed API methods for all OneBot v11 APIs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotBot;
//! use alloy_core::{BoxedBot, EventContext, FromContext};
//!
//! async fn my_handler(bot: BoxedBot, event: EventContext<MessageEvent>) {
//!     // Downcast to OneBotBot for strongly-typed APIs
//!     if let Some(onebot) = bot.as_any().downcast_ref::<OneBotBot>() {
//!         // Send a private message
//!         onebot.send_private_msg(12345678, "Hello!", false).await.ok();
//!         
//!         // Or use the generic send method
//!         bot.send(event.root.as_ref(), "Reply!").await.ok();
//!     }
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, trace};

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
    pending_calls: Arc<RwLock<HashMap<String, oneshot::Sender<Value>>>>,
    /// Echo counter for generating unique echo IDs.
    echo_counter: AtomicU64,
    /// API call timeout duration.
    api_timeout: Duration,
}

impl OneBotBot {
    /// Creates a new OneBotBot.
    pub fn new(id: impl Into<String>, connection: ConnectionHandle) -> Arc<Self> {
        Arc::new(Self {
            id: id.into(),
            connection,
            pending_calls: Arc::new(RwLock::new(HashMap::new())),
            echo_counter: AtomicU64::new(1),
            api_timeout: Duration::from_secs(30),
        })
    }

    /// Sets the API call timeout.
    pub fn with_timeout(self: Arc<Self>, timeout: Duration) -> Arc<Self> {
        // We need to recreate since we can't mutate through Arc
        Arc::new(Self {
            id: self.id.clone(),
            connection: self.connection.clone(),
            pending_calls: Arc::clone(&self.pending_calls),
            echo_counter: AtomicU64::new(self.echo_counter.load(Ordering::SeqCst)),
            api_timeout: timeout,
        })
    }

    /// Generates a unique echo ID for API calls.
    fn next_echo(&self) -> String {
        let counter = self.echo_counter.fetch_add(1, Ordering::SeqCst);
        format!("alloy_{counter}")
    }

    /// Internal method to call an API and wait for response.
    async fn call_api_internal(&self, action: &str, params: Value) -> ApiResult<Value> {
        let echo = self.next_echo();

        // Create channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending call
        {
            let mut pending = self.pending_calls.write().await;
            pending.insert(echo.clone(), tx);
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
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| ApiError::SerializationError(e.to_string()))?;

        self.connection
            .send(request_bytes)
            .await
            .map_err(|e| ApiError::TransportError(e.to_string()))?;

        // Wait for response with timeout
        match timeout(self.api_timeout, rx).await {
            Ok(Ok(response)) => {
                trace!(response = %response, "API response");

                // Check for API error
                if let Some(retcode) = response.get("retcode").and_then(Value::as_i64)
                    && retcode != 0
                {
                    let message = response
                        .get("msg")
                        .or_else(|| response.get("wording"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error")
                        .to_string();
                    return Err(ApiError::ApiError {
                        retcode: retcode as i32,
                        message,
                    });
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
                let mut pending = self.pending_calls.write().await;
                pending.remove(&echo);
                Err(ApiError::Timeout)
            }
        }
    }

    /// Handles an incoming response message.
    ///
    /// This should be called by the adapter when receiving messages from the server.
    pub async fn handle_response(&self, response: &Value) {
        if let Some(echo) = response.get("echo").and_then(|v| v.as_str()) {
            let mut pending = self.pending_calls.write().await;
            if let Some(tx) = pending.remove(echo) {
                let _ = tx.send(response.clone());
            }
        }
    }

    /// Clears all pending API calls.
    ///
    /// This should be called when the connection is lost to prevent memory leaks
    /// and to notify waiting callers that the connection was lost.
    ///
    /// All pending calls will receive a channel closed error, which will be
    /// converted to `ApiError::NotConnected`.
    pub async fn clear_pending_calls(&self) {
        let mut pending = self.pending_calls.write().await;
        let count = pending.len();
        if count > 0 {
            debug!(
                count = count,
                "Clearing pending API calls due to disconnect"
            );
            pending.clear();
            // Senders will be dropped, causing receivers to get an error
        }
    }

    /// Returns the number of pending API calls.
    ///
    /// This can be useful for monitoring or debugging purposes.
    pub async fn pending_call_count(&self) -> usize {
        self.pending_calls.read().await.len()
    }

    /// Returns a reference to self as Any for downcasting.
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    // =========================================================================
    // Message APIs
    // =========================================================================

    /// Sends a private message.
    ///
    /// # Arguments
    /// * `user_id` - Target user's QQ number
    /// * `message` - Message content (CQ code or plain text)
    /// * `auto_escape` - Whether to treat message as plain text (not parse CQ codes)
    pub async fn send_private_msg(
        &self,
        user_id: i64,
        message: impl Into<String>,
        auto_escape: bool,
    ) -> ApiResult<i64> {
        let result = self
            .call_api_internal(
                "send_private_msg",
                json!({
                    "user_id": user_id,
                    "message": message.into(),
                    "auto_escape": auto_escape
                }),
            )
            .await?;

        result
            .get("message_id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ApiError::SerializationError("Missing message_id".into()))
    }

    /// Sends a group message.
    ///
    /// # Arguments
    /// * `group_id` - Target group number
    /// * `message` - Message content (CQ code or plain text)
    /// * `auto_escape` - Whether to treat message as plain text (not parse CQ codes)
    pub async fn send_group_msg(
        &self,
        group_id: i64,
        message: impl Into<String>,
        auto_escape: bool,
    ) -> ApiResult<i64> {
        let result = self
            .call_api_internal(
                "send_group_msg",
                json!({
                    "group_id": group_id,
                    "message": message.into(),
                    "auto_escape": auto_escape
                }),
            )
            .await?;

        result
            .get("message_id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ApiError::SerializationError("Missing message_id".into()))
    }

    /// Sends a message (auto-detect type based on parameters).
    ///
    /// If both `user_id` and `group_id` are provided, `message_type` determines which to use.
    pub async fn send_msg(
        &self,
        message_type: Option<&str>,
        user_id: Option<i64>,
        group_id: Option<i64>,
        message: impl Into<String>,
        auto_escape: bool,
    ) -> ApiResult<i64> {
        let mut params = json!({
            "message": message.into(),
            "auto_escape": auto_escape
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

        let result = self.call_api_internal("send_msg", params).await?;

        result
            .get("message_id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ApiError::SerializationError("Missing message_id".into()))
    }

    /// Deletes (recalls) a message.
    pub async fn delete_msg(&self, message_id: i32) -> ApiResult<()> {
        self.call_api_internal(
            "delete_msg",
            json!({
                "message_id": message_id
            }),
        )
        .await?;
        Ok(())
    }

    /// Gets a message by ID.
    pub async fn get_msg(&self, message_id: i32) -> ApiResult<GetMsgResponse> {
        let result = self
            .call_api_internal(
                "get_msg",
                json!({
                    "message_id": message_id
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets a forwarded message.
    pub async fn get_forward_msg(&self, id: &str) -> ApiResult<Value> {
        self.call_api_internal(
            "get_forward_msg",
            json!({
                "id": id
            }),
        )
        .await
    }

    /// Sends a like (up to 10 times per day per friend).
    pub async fn send_like(&self, user_id: i64, times: u8) -> ApiResult<()> {
        self.call_api_internal(
            "send_like",
            json!({
                "user_id": user_id,
                "times": times.min(10)
            }),
        )
        .await?;
        Ok(())
    }

    // =========================================================================
    // Group Management APIs
    // =========================================================================

    /// Kicks a user from a group.
    pub async fn set_group_kick(
        &self,
        group_id: i64,
        user_id: i64,
        reject_add_request: bool,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_kick",
            json!({
                "group_id": group_id,
                "user_id": user_id,
                "reject_add_request": reject_add_request
            }),
        )
        .await?;
        Ok(())
    }

    /// Bans a user in a group.
    ///
    /// # Arguments
    /// * `group_id` - Group number
    /// * `user_id` - User to ban
    /// * `duration` - Ban duration in seconds (0 = unban)
    pub async fn set_group_ban(&self, group_id: i64, user_id: i64, duration: u32) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_ban",
            json!({
                "group_id": group_id,
                "user_id": user_id,
                "duration": duration
            }),
        )
        .await?;
        Ok(())
    }

    /// Bans an anonymous user in a group.
    pub async fn set_group_anonymous_ban(
        &self,
        group_id: i64,
        anonymous_flag: &str,
        duration: u32,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_anonymous_ban",
            json!({
                "group_id": group_id,
                "anonymous_flag": anonymous_flag,
                "duration": duration
            }),
        )
        .await?;
        Ok(())
    }

    /// Enables/disables whole group ban.
    pub async fn set_group_whole_ban(&self, group_id: i64, enable: bool) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_whole_ban",
            json!({
                "group_id": group_id,
                "enable": enable
            }),
        )
        .await?;
        Ok(())
    }

    /// Sets/unsets a user as group admin.
    pub async fn set_group_admin(
        &self,
        group_id: i64,
        user_id: i64,
        enable: bool,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_admin",
            json!({
                "group_id": group_id,
                "user_id": user_id,
                "enable": enable
            }),
        )
        .await?;
        Ok(())
    }

    /// Enables/disables anonymous chat in a group.
    pub async fn set_group_anonymous(&self, group_id: i64, enable: bool) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_anonymous",
            json!({
                "group_id": group_id,
                "enable": enable
            }),
        )
        .await?;
        Ok(())
    }

    /// Sets a user's group card (nickname).
    pub async fn set_group_card(&self, group_id: i64, user_id: i64, card: &str) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_card",
            json!({
                "group_id": group_id,
                "user_id": user_id,
                "card": card
            }),
        )
        .await?;
        Ok(())
    }

    /// Sets the group name.
    pub async fn set_group_name(&self, group_id: i64, group_name: &str) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_name",
            json!({
                "group_id": group_id,
                "group_name": group_name
            }),
        )
        .await?;
        Ok(())
    }

    /// Leaves a group.
    pub async fn set_group_leave(&self, group_id: i64, is_dismiss: bool) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_leave",
            json!({
                "group_id": group_id,
                "is_dismiss": is_dismiss
            }),
        )
        .await?;
        Ok(())
    }

    /// Sets a user's special title in a group.
    pub async fn set_group_special_title(
        &self,
        group_id: i64,
        user_id: i64,
        special_title: &str,
        duration: i32,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_special_title",
            json!({
                "group_id": group_id,
                "user_id": user_id,
                "special_title": special_title,
                "duration": duration
            }),
        )
        .await?;
        Ok(())
    }

    // =========================================================================
    // Friend/Group Request APIs
    // =========================================================================

    /// Handles a friend add request.
    pub async fn set_friend_add_request(
        &self,
        flag: &str,
        approve: bool,
        remark: &str,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_friend_add_request",
            json!({
                "flag": flag,
                "approve": approve,
                "remark": remark
            }),
        )
        .await?;
        Ok(())
    }

    /// Handles a group add/invite request.
    pub async fn set_group_add_request(
        &self,
        flag: &str,
        sub_type: &str,
        approve: bool,
        reason: &str,
    ) -> ApiResult<()> {
        self.call_api_internal(
            "set_group_add_request",
            json!({
                "flag": flag,
                "sub_type": sub_type,
                "approve": approve,
                "reason": reason
            }),
        )
        .await?;
        Ok(())
    }

    // =========================================================================
    // Information APIs
    // =========================================================================

    /// Gets login info.
    pub async fn get_login_info(&self) -> ApiResult<LoginInfo> {
        let result = self.call_api_internal("get_login_info", json!({})).await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets stranger info.
    pub async fn get_stranger_info(&self, user_id: i64, no_cache: bool) -> ApiResult<StrangerInfo> {
        let result = self
            .call_api_internal(
                "get_stranger_info",
                json!({
                    "user_id": user_id,
                    "no_cache": no_cache
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets the friend list.
    pub async fn get_friend_list(&self) -> ApiResult<Vec<FriendInfo>> {
        let result = self.call_api_internal("get_friend_list", json!({})).await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets group info.
    pub async fn get_group_info(&self, group_id: i64, no_cache: bool) -> ApiResult<GroupInfo> {
        let result = self
            .call_api_internal(
                "get_group_info",
                json!({
                    "group_id": group_id,
                    "no_cache": no_cache
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets the group list.
    pub async fn get_group_list(&self) -> ApiResult<Vec<GroupInfo>> {
        let result = self.call_api_internal("get_group_list", json!({})).await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets group member info.
    pub async fn get_group_member_info(
        &self,
        group_id: i64,
        user_id: i64,
        no_cache: bool,
    ) -> ApiResult<GroupMemberInfo> {
        let result = self
            .call_api_internal(
                "get_group_member_info",
                json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "no_cache": no_cache
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets the group member list.
    pub async fn get_group_member_list(&self, group_id: i64) -> ApiResult<Vec<GroupMemberInfo>> {
        let result = self
            .call_api_internal(
                "get_group_member_list",
                json!({
                    "group_id": group_id
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets group honor info.
    pub async fn get_group_honor_info(&self, group_id: i64, honor_type: &str) -> ApiResult<Value> {
        self.call_api_internal(
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

    /// Gets cookies for a domain.
    pub async fn get_cookies(&self, domain: &str) -> ApiResult<String> {
        let result = self
            .call_api_internal(
                "get_cookies",
                json!({
                    "domain": domain
                }),
            )
            .await?;
        result
            .get("cookies")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ApiError::SerializationError("Missing cookies".into()))
    }

    /// Gets CSRF token.
    pub async fn get_csrf_token(&self) -> ApiResult<i64> {
        let result = self.call_api_internal("get_csrf_token", json!({})).await?;
        result
            .get("token")
            .and_then(Value::as_i64)
            .ok_or_else(|| ApiError::SerializationError("Missing token".into()))
    }

    /// Gets credentials (cookies + CSRF token).
    pub async fn get_credentials(&self, domain: &str) -> ApiResult<Credentials> {
        let result = self
            .call_api_internal(
                "get_credentials",
                json!({
                    "domain": domain
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    // =========================================================================
    // File APIs
    // =========================================================================

    /// Gets a voice file.
    pub async fn get_record(&self, file: &str, out_format: &str) -> ApiResult<String> {
        let result = self
            .call_api_internal(
                "get_record",
                json!({
                    "file": file,
                    "out_format": out_format
                }),
            )
            .await?;
        result
            .get("file")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ApiError::SerializationError("Missing file".into()))
    }

    /// Gets an image file.
    pub async fn get_image(&self, file: &str) -> ApiResult<String> {
        let result = self
            .call_api_internal(
                "get_image",
                json!({
                    "file": file
                }),
            )
            .await?;
        result
            .get("file")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ApiError::SerializationError("Missing file".into()))
    }

    /// Checks if the bot can send images.
    pub async fn can_send_image(&self) -> ApiResult<bool> {
        let result = self.call_api_internal("can_send_image", json!({})).await?;
        Ok(result.get("yes").and_then(Value::as_bool).unwrap_or(false))
    }

    /// Checks if the bot can send voice.
    pub async fn can_send_record(&self) -> ApiResult<bool> {
        let result = self.call_api_internal("can_send_record", json!({})).await?;
        Ok(result.get("yes").and_then(Value::as_bool).unwrap_or(false))
    }

    // =========================================================================
    // System APIs
    // =========================================================================

    /// Gets the running status.
    pub async fn get_status(&self) -> ApiResult<Status> {
        let result = self.call_api_internal("get_status", json!({})).await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Gets version info.
    pub async fn get_version_info(&self) -> ApiResult<VersionInfo> {
        let result = self
            .call_api_internal("get_version_info", json!({}))
            .await?;
        serde_json::from_value(result).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Restarts the OneBot implementation.
    pub async fn set_restart(&self, delay: u32) -> ApiResult<()> {
        self.call_api_internal(
            "set_restart",
            json!({
                "delay": delay
            }),
        )
        .await?;
        Ok(())
    }

    /// Cleans the cache.
    pub async fn clean_cache(&self) -> ApiResult<()> {
        self.call_api_internal("clean_cache", json!({})).await?;
        Ok(())
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

    fn adapter_name(&self) -> &'static str {
        "onebot"
    }

    async fn call_api(&self, action: &str, params: &str) -> ApiResult<Value> {
        let params: Value = serde_json::from_str(params)
            .map_err(|e| ApiError::SerializationError(e.to_string()))?;
        self.call_api_internal(action, params).await
    }

    async fn send(&self, event: &dyn Event, message: &str) -> ApiResult<i64> {
        // Try to extract session info from event
        let raw_json = event
            .raw_json()
            .ok_or_else(|| ApiError::MissingSession("No raw JSON in event".into()))?;

        let parsed: Value = serde_json::from_str(raw_json)
            .map_err(|e| ApiError::SerializationError(e.to_string()))?;

        // Check message type
        let message_type = parsed.get("message_type").and_then(|v| v.as_str());

        let result = match message_type {
            Some("private") => {
                let user_id = parsed
                    .get("user_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| ApiError::MissingSession("No user_id in event".into()))?;
                self.send_private_msg(user_id, message, false).await?
            }
            Some("group") => {
                let group_id = parsed
                    .get("group_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| ApiError::MissingSession("No group_id in event".into()))?;
                self.send_group_msg(group_id, message, false).await?
            }
            _ => {
                // Try to detect from available fields
                if let Some(group_id) = parsed.get("group_id").and_then(Value::as_i64) {
                    self.send_group_msg(group_id, message, false).await?
                } else if let Some(user_id) = parsed.get("user_id").and_then(Value::as_i64) {
                    self.send_private_msg(user_id, message, false).await?
                } else {
                    return Err(ApiError::MissingSession(
                        "Cannot determine message target".into(),
                    ));
                }
            }
        };

        Ok(result)
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

// =============================================================================
// Response Types
// =============================================================================

/// Response from get_msg API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMsgResponse {
    pub time: i32,
    pub message_type: String,
    pub message_id: i32,
    pub real_id: i32,
    pub sender: Value,
    pub message: Value,
}

/// Login info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginInfo {
    pub user_id: i64,
    pub nickname: String,
}

/// Stranger info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrangerInfo {
    pub user_id: i64,
    pub nickname: String,
    pub sex: String,
    pub age: i32,
}

/// Friend info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInfo {
    pub user_id: i64,
    pub nickname: String,
    pub remark: String,
}

/// Group info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub group_id: i64,
    pub group_name: String,
    pub member_count: i32,
    pub max_member_count: i32,
}

/// Group member info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberInfo {
    pub group_id: i64,
    pub user_id: i64,
    pub nickname: String,
    pub card: String,
    pub sex: String,
    pub age: i32,
    #[serde(default)]
    pub area: String,
    pub join_time: i32,
    pub last_sent_time: i32,
    pub level: String,
    pub role: String,
    pub unfriendly: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub title_expire_time: i32,
    pub card_changeable: bool,
}

/// Credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub cookies: String,
    pub csrf_token: i32,
}

/// Status info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub online: Option<bool>,
    pub good: bool,
}

/// Version info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub app_name: String,
    pub app_version: String,
    pub protocol_version: String,
}
