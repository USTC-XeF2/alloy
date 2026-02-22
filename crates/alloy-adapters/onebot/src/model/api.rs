//! API request and response types for OneBot v11.
//!
//! This module defines the structures for making API calls to the OneBot implementation.

use serde::{Deserialize, Serialize};

use super::message::OneBotMessage;
use super::types::Sender;

/// Response from get_msg API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMsgResponse {
    pub time: i32,
    pub message_type: String,
    pub message_id: i32,
    pub real_id: i32,
    pub sender: Sender,
    #[serde(with = "super::message::serde_message")]
    pub message: OneBotMessage,
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
