use serde::Serialize;

use crate::bot::OneBotBot;
use crate::model::api::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo, GroupMemberInfo, LoginInfo, MessageType,
    StrangerInfo, VersionInfo,
};
use crate::model::message::OneBotMessage;
use crate::model::types::{GroupRequestType, Status};
use amira_macros::api_payload;

/// Sends a private message.
#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = i32, field = "message_id")]
pub struct SendPrivateMsg {
    pub user_id: i64,
    #[api_param(into)]
    pub message: OneBotMessage,
    #[api_param(default)]
    pub auto_escape: bool,
}

/// Sends a group message.
#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = i32, field = "message_id")]
pub struct SendGroupMsg {
    pub group_id: i64,
    #[api_param(into)]
    pub message: OneBotMessage,
    #[api_param(default)]
    pub auto_escape: bool,
}

/// Sends a message (auto-detect type based on parameters).
///
/// If both `user_id` and `group_id` are provided, `message_type` determines which to use.
#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = i64, field = "message_id")]
pub struct SendMsg {
    #[api_param(into)]
    pub message: OneBotMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub message_type: Option<MessageType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub user_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub group_id: Option<i64>,
}

/// Deletes (recalls) a message.
#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct DeleteMsg {
    pub message_id: i32,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = GetMsgResponse)]
pub struct GetMsg {
    pub message_id: i32,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = OneBotMessage, field = "message")]
pub struct GetForwardMsg {
    #[api_param(into)]
    pub id: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SendLike {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub times: Option<u8>,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupKick {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default)]
    pub reject_add_request: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupBan {
    pub group_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub duration: Option<u32>,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupWholeBan {
    pub group_id: i64,
    #[api_param(default = "default_true")]
    pub enable: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupAdmin {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default = "default_true")]
    pub enable: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupCard {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(into, default)]
    pub card: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupName {
    pub group_id: i64,
    #[api_param(into)]
    pub group_name: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupLeave {
    pub group_id: i64,
    #[api_param(default)]
    pub is_dismiss: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupSpecialTitle {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(into, default)]
    pub special_title: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetFriendAddRequest {
    #[api_param(into)]
    pub flag: String,
    #[api_param(default = "default_true")]
    pub approve: bool,
    #[api_param(into, default)]
    pub remark: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetGroupAddRequest {
    #[api_param(into)]
    pub flag: String,
    #[api_param(into)]
    pub sub_type: GroupRequestType,
    #[api_param(default = "default_true")]
    pub approve: bool,
    #[api_param(into, default)]
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = LoginInfo)]
pub struct GetLoginInfo {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = StrangerInfo)]
pub struct GetStrangerInfo {
    pub user_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = Vec<FriendInfo>)]
pub struct GetFriendList {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = GroupInfo)]
pub struct GetGroupInfo {
    pub group_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = Vec<GroupInfo>)]
pub struct GetGroupList {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = GroupMemberInfo)]
pub struct GetGroupMemberInfo {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = Vec<GroupMemberInfo>)]
pub struct GetGroupMemberList {
    pub group_id: i64,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = serde_json::Value)]
pub struct GetGroupHonorInfo {
    pub group_id: i64,
    #[serde(rename = "type")]
    #[api_param(into)]
    pub honor_type: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = String, field = "cookies")]
pub struct GetCookies {
    #[api_param(into, default)]
    pub domain: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = i32, field = "token")]
pub struct GetCsrfToken {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = Credentials)]
pub struct GetCredentials {
    #[api_param(into, default)]
    pub domain: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = String, field = "file")]
pub struct GetRecord {
    #[api_param(into)]
    pub file: String,
    #[api_param(into)]
    pub out_format: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = String, field = "file")]
pub struct GetImage {
    #[api_param(into)]
    pub file: String,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = bool, field = "yes")]
pub struct CanSendImage {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = bool, field = "yes")]
pub struct CanSendRecord {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = Status)]
pub struct GetStatus {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot, response = VersionInfo)]
pub struct GetVersionInfo {}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct SetRestart {
    #[api_param(default)]
    pub delay: u32,
}

#[derive(Debug, Serialize)]
#[api_payload(bot = OneBotBot)]
pub struct CleanCache {}
