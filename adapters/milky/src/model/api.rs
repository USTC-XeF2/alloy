//! Milky protocol API types.

use amira_core::Message;
use serde::{Deserialize, Deserializer, Serialize};

use super::entity::{
    FriendEntity, GroupEntity, GroupFileEntity, GroupFolderEntity, GroupMemberEntity,
    GroupNotification, Sex,
};
use super::message::IncomingSegment;

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub(crate) enum ApiResponse<T> {
    Ok {
        #[serde(bound(deserialize = "T: Deserialize<'de>"))]
        #[serde(deserialize_with = "try_unit_first")]
        data: T,
    },
    Failed {
        retcode: i64,
        message: String,
    },
}

fn try_unit_first<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    if let Ok(val) = T::deserialize(serde::de::value::UnitDeserializer::<D::Error>::new()) {
        serde::de::IgnoredAny::deserialize(deserializer)?;
        return Ok(val);
    }

    T::deserialize(deserializer)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupRequestType {
    JoinRequest,
    InvitedJoinRequest,
}

/// Response for `get_login_info`.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginInfo {
    pub uin: i64,
    pub nickname: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Windows,
    Linux,
    MacOS,
    #[serde(rename = "android_pad")]
    AndroidPad,
    #[serde(rename = "android_phone")]
    AndroidPhone,
    IPad,
    IPhone,
    Harmony,
    Watch,
}

/// Response for `get_impl_info`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImplInfo {
    pub impl_name: String,
    pub impl_version: String,
    pub qq_protocol_version: String,
    pub qq_protocol_type: ProtocolType,
    pub milky_version: String,
}

/// Response for `get_user_profile`.
#[derive(Debug, Clone, Deserialize)]
pub struct UserProfile {
    pub nickname: String,
    pub qid: String,
    pub age: i32,
    pub sex: Sex,
    pub remark: String,
    pub bio: String,
    pub level: i32,
    pub country: String,
    pub city: String,
    pub school: String,
}

/// Response for `get_peer_pins`.
#[cfg(feature = "v1_2")]
#[derive(Debug, Clone, Deserialize)]
pub struct PeerPinsResponse {
    pub friends: Vec<FriendEntity>,
    pub groups: Vec<GroupEntity>,
}

/// Response for `send_private_message` and `send_group_message`.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageResponse {
    pub message_seq: i64,
    pub time: i64,
}

/// An incoming message (from `get_message` or `get_history_messages`).
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingMessage {
    pub message_scene: String,
    pub peer_id: i64,
    pub message_seq: i64,
    pub sender_id: i64,
    pub time: i64,
    pub segments: Message<IncomingSegment>,
    #[serde(default)]
    pub friend: Option<FriendEntity>,
    #[serde(default)]
    pub group: Option<GroupEntity>,
    #[serde(default)]
    pub group_member: Option<GroupMemberEntity>,
}

/// An incoming forwarded message (from `get_forwarded_messages`).
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingForwardedMessage {
    pub sender_name: String,
    pub avatar_url: String,
    pub time: i64,
    pub segments: Message<IncomingSegment>,
}

/// Response for `get_history_messages`.
#[derive(Debug, Clone, Deserialize)]
pub struct HistoryMessagesResponse {
    pub messages: Vec<IncomingMessage>,
    #[serde(default)]
    pub next_message_seq: Option<i64>,
}

/// An incoming group essence message (from `get_group_essence_messages`).
#[derive(Debug, Clone, Deserialize)]
pub struct GroupEssenceMessage {
    pub group_id: i64,
    pub message_seq: i64,
    pub message_time: i64,
    pub sender_id: i64,
    pub sender_name: String,
    pub operator_id: i64,
    pub operator_name: String,
    pub operation_time: i64,
    pub segments: Message<IncomingSegment>,
}

/// Response for `get_group_essence_messages`.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupEssenceMessagesResponse {
    pub messages: Vec<GroupEssenceMessage>,
    pub is_end: bool,
}

/// Response for `get_group_notifications`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupNotificationsResponse {
    pub notifications: Vec<GroupNotification>,
    #[serde(default)]
    pub next_notification_seq: Option<i64>,
}

/// Response for `get_group_files`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupFilesResponse {
    pub files: Vec<GroupFileEntity>,
    pub folders: Vec<GroupFolderEntity>,
}
