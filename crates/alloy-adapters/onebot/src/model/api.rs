//! API request and response types for OneBot v11.
//!
//! This module defines the structures for making API calls to the OneBot implementation.

use serde::{Deserialize, Serialize};

use super::segment::Segment;

/// Sender information in API responses.
///
/// This is a simplified sender type that works for both private and group messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiSender {
    /// The sender's QQ number.
    #[serde(default)]
    pub user_id: i64,
    /// The sender's nickname.
    #[serde(default)]
    pub nickname: String,
    /// The sender's sex (male, female, unknown).
    #[serde(default)]
    pub sex: String,
    /// The sender's age.
    #[serde(default)]
    pub age: i32,
    /// Group card (for group messages).
    #[serde(default)]
    pub card: Option<String>,
    /// Group level (for group messages).
    #[serde(default)]
    pub level: Option<String>,
    /// Group role (for group messages).
    #[serde(default)]
    pub role: Option<String>,
    /// Group title (for group messages).
    #[serde(default)]
    pub title: Option<String>,
}

/// A generic API response from OneBot v11.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// The status: "ok", "async", or "failed".
    pub status: String,
    /// The return code (0 for success).
    pub retcode: i32,
    /// The response data (if successful).
    pub data: Option<T>,
    /// Error message (if failed).
    #[serde(default)]
    pub msg: Option<String>,
    /// Additional error info.
    #[serde(default)]
    pub wording: Option<String>,
    /// Echo data from the request.
    #[serde(default)]
    pub echo: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Checks if the API call was successful.
    pub fn is_ok(&self) -> bool {
        self.status == "ok" && self.retcode == 0
    }

    /// Converts the response into a Result.
    pub fn into_result(self) -> anyhow::Result<T> {
        if self.is_ok() {
            self.data
                .ok_or_else(|| anyhow::anyhow!("No data in response"))
        } else {
            Err(anyhow::anyhow!(
                "API error: {} (code: {})",
                self.msg.unwrap_or_else(|| "Unknown error".into()),
                self.retcode
            ))
        }
    }
}

/// Parameters for sending a private message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPrivateMsgParams {
    /// The target user's QQ number.
    pub user_id: i64,
    /// The message content.
    pub message: Vec<Segment>,
    /// Whether to auto-escape CQ codes.
    #[serde(default)]
    pub auto_escape: bool,
}

/// Parameters for sending a group message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupMsgParams {
    /// The target group ID.
    pub group_id: i64,
    /// The message content.
    pub message: Vec<Segment>,
    /// Whether to auto-escape CQ codes.
    #[serde(default)]
    pub auto_escape: bool,
}

/// Parameters for deleting a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMsgParams {
    /// The message ID to delete.
    pub message_id: i32,
}

/// Parameters for getting message info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMsgParams {
    /// The message ID.
    pub message_id: i32,
}

/// Response data for send_msg API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMsgResponse {
    /// The ID of the sent message.
    pub message_id: i32,
}

/// Response data for get_msg API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMsgResponse {
    /// The message ID.
    pub message_id: i32,
    /// The real message ID.
    #[serde(default)]
    pub real_id: Option<i32>,
    /// The sender information.
    pub sender: ApiSender,
    /// The time the message was sent.
    pub time: i64,
    /// The message content.
    pub message: Vec<Segment>,
    /// The raw message string.
    pub raw_message: String,
}

/// Parameters for setting group kick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupKickParams {
    /// The group ID.
    pub group_id: i64,
    /// The user ID to kick.
    pub user_id: i64,
    /// Whether to reject future join requests.
    #[serde(default)]
    pub reject_add_request: bool,
}

/// Parameters for setting group ban (mute).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupBanParams {
    /// The group ID.
    pub group_id: i64,
    /// The user ID to mute.
    pub user_id: i64,
    /// The duration in seconds (0 to unmute).
    #[serde(default)]
    pub duration: u64,
}

/// Parameters for setting whole group ban.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupWholeBanParams {
    /// The group ID.
    pub group_id: i64,
    /// Whether to enable whole group mute.
    #[serde(default)]
    pub enable: bool,
}

/// Parameters for setting group name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupNameParams {
    /// The group ID.
    pub group_id: i64,
    /// The new group name.
    pub group_name: String,
}

/// Parameters for getting login info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLoginInfoResponse {
    /// The bot's QQ number.
    pub user_id: i64,
    /// The bot's nickname.
    pub nickname: String,
}

/// Parameters for getting stranger info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStrangerInfoParams {
    /// The target user's QQ number.
    pub user_id: i64,
    /// Whether to skip cache.
    #[serde(default)]
    pub no_cache: bool,
}

/// Response for stranger info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrangerInfo {
    /// The user's QQ number.
    pub user_id: i64,
    /// The user's nickname.
    pub nickname: String,
    /// The user's sex.
    #[serde(default)]
    pub sex: String,
    /// The user's age.
    #[serde(default)]
    pub age: i32,
}

/// Parameters for getting group info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupInfoParams {
    /// The group ID.
    pub group_id: i64,
    /// Whether to skip cache.
    #[serde(default)]
    pub no_cache: bool,
}

/// Response for group info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    /// The group ID.
    pub group_id: i64,
    /// The group name.
    pub group_name: String,
    /// The group memo.
    #[serde(default)]
    pub group_memo: String,
    /// The group creation time.
    #[serde(default)]
    pub group_create_time: u64,
    /// The group level.
    #[serde(default)]
    pub group_level: u32,
    /// The member count.
    #[serde(default)]
    pub member_count: i32,
    /// The maximum member count.
    #[serde(default)]
    pub max_member_count: i32,
}

/// Parameters for getting group member info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupMemberInfoParams {
    /// The group ID.
    pub group_id: i64,
    /// The user ID.
    pub user_id: i64,
    /// Whether to skip cache.
    #[serde(default)]
    pub no_cache: bool,
}

/// Response for group member info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberInfo {
    /// The group ID.
    pub group_id: i64,
    /// The user's QQ number.
    pub user_id: i64,
    /// The user's nickname.
    pub nickname: String,
    /// The user's group card.
    #[serde(default)]
    pub card: String,
    /// The user's sex.
    #[serde(default)]
    pub sex: String,
    /// The user's age.
    #[serde(default)]
    pub age: i32,
    /// The user's area.
    #[serde(default)]
    pub area: String,
    /// The time the user joined the group.
    #[serde(default)]
    pub join_time: i64,
    /// The time of the user's last message.
    #[serde(default)]
    pub last_sent_time: i64,
    /// The user's level in the group.
    #[serde(default)]
    pub level: String,
    /// The user's role: "owner", "admin", or "member".
    #[serde(default)]
    pub role: String,
    /// Whether the user can be unfriended.
    #[serde(default)]
    pub unfriendly: bool,
    /// The user's group title.
    #[serde(default)]
    pub title: String,
    /// The expiration time of the title.
    #[serde(default)]
    pub title_expire_time: i64,
    /// Whether the user's card can be changed.
    #[serde(default)]
    pub card_changeable: bool,
}

/// A generic API request.
#[derive(Debug, Clone, Serialize)]
pub struct ApiRequest<T: Serialize> {
    /// The action name.
    pub action: String,
    /// The parameters.
    pub params: T,
    /// Optional echo data for matching responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<String>,
}

impl<T: Serialize> ApiRequest<T> {
    /// Creates a new API request.
    pub fn new(action: impl Into<String>, params: T) -> Self {
        Self {
            action: action.into(),
            params,
            echo: None,
        }
    }

    /// Sets the echo field for response matching.
    pub fn with_echo(mut self, echo: impl Into<String>) -> Self {
        self.echo = Some(echo.into());
        self
    }
}
