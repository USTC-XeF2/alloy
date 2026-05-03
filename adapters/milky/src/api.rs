use serde::Serialize;

use crate::bot::MilkyBot;
#[cfg(feature = "v1_2")]
use crate::model::api::PeerPinsResponse;
use crate::model::api::{
    GroupEssenceMessagesResponse, GroupFilesResponse, GroupNotificationsResponse, GroupRequestType,
    HistoryMessagesResponse, ImplInfo, IncomingForwardedMessage, IncomingMessage, LoginInfo,
    SendMessageResponse, UserProfile,
};
#[cfg(feature = "v1_2")]
use crate::model::entity::ReactionType;
use crate::model::entity::{
    FriendEntity, FriendRequest, GroupAnnouncementEntity, GroupEntity, GroupMemberEntity,
    MessageSceneType,
};
use crate::model::message::OutgoingSegment;
use amira_core::Message;
use amira_macros::api_payload;

// =========================================================================
// Account APIs
// =========================================================================

/// Gets login info (QQ number and nickname).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = LoginInfo)]
pub struct GetLoginInfo {}

/// Gets implementation info (version, protocol, etc.).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = ImplInfo)]
pub struct GetImplInfo {}

/// Gets a user's profile.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = UserProfile)]
pub struct GetUserProfile {
    pub user_id: i64,
}

/// Gets the friend list.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<FriendEntity>, field = "friends")]
pub struct GetFriendList {
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets friend info.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = FriendEntity, field = "friend")]
pub struct GetFriendInfo {
    pub user_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets the group list.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<GroupEntity>, field = "groups")]
pub struct GetGroupList {
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets group info.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = GroupEntity, field = "group")]
pub struct GetGroupInfo {
    pub group_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets group member list.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<GroupMemberEntity>, field = "members")]
pub struct GetGroupMemberList {
    pub group_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets group member info.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = GroupMemberEntity, field = "member")]
pub struct GetGroupMemberInfo {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default)]
    pub no_cache: bool,
}

/// Gets pinned (top) friends or groups.
#[cfg(feature = "v1_2")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = PeerPinsResponse, field = "pins")]
pub struct GetPeerPins {}

/// Sets a friend or group as pinned (top).
#[cfg(feature = "v1_2")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetPeerPin {
    pub message_scene: MessageSceneType,
    pub peer_id: i64,
    #[api_param(default = "default_true")]
    pub is_pinned: bool,
}

const fn default_true() -> bool {
    true
}

/// Sets the bot's avatar.
#[cfg(feature = "v1_1")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetAvatar {
    #[api_param(into)]
    pub uri: String,
}

/// Sets the bot's nickname.
#[cfg(feature = "v1_1")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetNickname {
    #[api_param(into)]
    pub new_nickname: String,
}

/// Sets the bot's bio/signature.
#[cfg(feature = "v1_1")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetBio {
    #[api_param(into)]
    pub new_bio: String,
}

/// Gets custom face/sticker URL list.
#[cfg(feature = "v1_1")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<String>, field = "urls")]
pub struct GetCustomFaceUrlList {}

/// Gets cookies for a domain.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "cookies")]
pub struct GetCookies {
    #[api_param(into)]
    pub domain: String,
}

/// Gets CSRF token.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "csrf_token")]
pub struct GetCsrfToken {}

// =========================================================================
// Message APIs
// =========================================================================

/// Sends a private (friend) message.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = SendMessageResponse)]
pub struct SendPrivateMessage {
    pub user_id: i64,
    #[api_param(into)]
    pub message: Message<OutgoingSegment>,
}

/// Sends a group message.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = SendMessageResponse)]
pub struct SendGroupMessage {
    pub group_id: i64,
    #[api_param(into)]
    pub message: Message<OutgoingSegment>,
}

/// Recalls a private message.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RecallPrivateMessage {
    pub user_id: i64,
    pub message_seq: i64,
}

/// Recalls a group message.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RecallGroupMessage {
    pub group_id: i64,
    pub message_seq: i64,
}

/// Gets a message.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = IncomingMessage, field = "message")]
pub struct GetMessage {
    pub message_scene: MessageSceneType,
    pub peer_id: i64,
    pub message_seq: i64,
}

/// Gets history messages.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = HistoryMessagesResponse)]
pub struct GetHistoryMessages {
    pub message_scene: MessageSceneType,
    pub peer_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub start_message_seq: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub limit: Option<i32>,
}

/// Gets a temporary resource URL.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "url")]
pub struct GetResourceTempUrl {
    #[api_param(into)]
    pub resource_id: String,
}

/// Gets forwarded messages.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<IncomingForwardedMessage>, field = "messages")]
pub struct GetForwardedMessages {
    #[api_param(into)]
    pub forward_id: String,
}

/// Marks messages as read.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct MarkMessageAsRead {
    pub message_scene: MessageSceneType,
    pub peer_id: i64,
    pub message_seq: i64,
}

// =========================================================================
// Friend APIs
// =========================================================================

/// Sends a friend nudge (poke).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SendFriendNudge {
    pub user_id: i64,
    #[api_param(default)]
    pub is_self: bool,
}

/// Sends profile likes.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SendProfileLike {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub count: Option<i32>,
}

/// Deletes a friend.
#[cfg(feature = "v1_1")]
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct DeleteFriend {
    pub user_id: i64,
}

/// Gets pending friend requests.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<FriendRequest>, field = "requests")]
pub struct GetFriendRequests {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub limit: Option<i32>,
    #[api_param(default)]
    pub is_filtered: bool,
}

/// Accepts a friend request.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct AcceptFriendRequest {
    #[api_param(into)]
    pub initiator_uid: String,
    #[api_param(default)]
    pub is_filtered: bool,
}

/// Rejects a friend request.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RejectFriendRequest {
    #[api_param(into)]
    pub initiator_uid: String,
    #[api_param(default)]
    pub is_filtered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub reason: Option<String>,
}

// =========================================================================
// Group APIs
// =========================================================================

/// Sets group name.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupName {
    pub group_id: i64,
    #[api_param(into)]
    pub new_group_name: String,
}

/// Sets group avatar.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupAvatar {
    pub group_id: i64,
    #[api_param(into)]
    pub image_uri: String,
}

/// Sets a member's group card/nickname.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupMemberCard {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(into)]
    pub card: String,
}

/// Sets a member's special title.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupMemberSpecialTitle {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(into)]
    pub special_title: String,
}

/// Sets/unsets a member as group admin.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupMemberAdmin {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default = "default_true")]
    pub is_set: bool,
}

/// Mutes a group member.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupMemberMute {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default)]
    pub duration: i32,
}

/// Sets whole-group mute.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupWholeMute {
    pub group_id: i64,
    #[api_param(default = "default_true")]
    pub is_mute: bool,
}

/// Kicks a member from a group.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct KickGroupMember {
    pub group_id: i64,
    pub user_id: i64,
    #[api_param(default)]
    pub reject_add_request: bool,
}

/// Gets group announcements.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = Vec<GroupAnnouncementEntity>, field = "announcements")]
pub struct GetGroupAnnouncements {
    pub group_id: i64,
}

/// Sends a group announcement.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SendGroupAnnouncement {
    pub group_id: i64,
    #[api_param(into)]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub image_uri: Option<String>,
}

/// Deletes a group announcement.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct DeleteGroupAnnouncement {
    pub group_id: i64,
    #[api_param(into)]
    pub announcement_id: String,
}

/// Gets group essence messages.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = GroupEssenceMessagesResponse)]
pub struct GetGroupEssenceMessages {
    pub group_id: i64,
    pub page_index: i32,
    pub page_size: i32,
}

/// Sets a message as group essence.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SetGroupEssenceMessage {
    pub group_id: i64,
    pub message_seq: i64,
    #[api_param(default = "default_true")]
    pub is_set: bool,
}

/// Quits a group.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct QuitGroup {
    pub group_id: i64,
}

/// Sends a group message reaction (emoji response).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SendGroupMessageReaction {
    pub group_id: i64,
    pub message_seq: i64,
    #[api_param(into)]
    pub reaction: String,
    #[cfg(feature = "v1_2")]
    #[api_param(default)]
    pub reaction_type: ReactionType,
    #[api_param(default = "default_true")]
    pub is_add: bool,
}

/// Sends a group nudge (poke).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct SendGroupNudge {
    pub group_id: i64,
    pub user_id: i64,
}

/// Gets group notifications (join requests, etc.).
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = GroupNotificationsResponse)]
pub struct GetGroupNotifications {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub start_notification_seq: Option<i64>,
    #[api_param(default)]
    pub is_filtered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub limit: Option<i32>,
}

/// Accepts a group join request.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct AcceptGroupRequest {
    pub notification_seq: i64,
    pub notification_type: GroupRequestType,
    pub group_id: i64,
    #[api_param(default)]
    pub is_filtered: bool,
}

/// Rejects a group join request.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RejectGroupRequest {
    pub notification_seq: i64,
    pub notification_type: GroupRequestType,
    pub group_id: i64,
    #[api_param(default)]
    pub is_filtered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub reason: Option<String>,
}

/// Accepts a group invitation.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct AcceptGroupInvitation {
    pub group_id: i64,
    pub invitation_seq: i64,
}

/// Rejects a group invitation.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RejectGroupInvitation {
    pub group_id: i64,
    pub invitation_seq: i64,
}

// =========================================================================
// File APIs
// =========================================================================

/// Uploads a private file.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "file_id")]
pub struct UploadPrivateFile {
    pub user_id: i64,
    #[api_param(into)]
    pub file_uri: String,
    #[api_param(into)]
    pub file_name: String,
}

/// Uploads a group file.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "file_id")]
pub struct UploadGroupFile {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub parent_folder_id: Option<String>,
    #[api_param(into)]
    pub file_uri: String,
    #[api_param(into)]
    pub file_name: String,
}

/// Gets a private file download URL.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "download_url")]
pub struct GetPrivateFileDownloadUrl {
    pub user_id: i64,
    #[api_param(into)]
    pub file_id: String,
    #[api_param(into)]
    pub file_hash: String,
}

/// Gets a group file download URL.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "download_url")]
pub struct GetGroupFileDownloadUrl {
    pub group_id: i64,
    #[api_param(into)]
    pub file_id: String,
}

/// Gets group files and folders.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = GroupFilesResponse)]
pub struct GetGroupFiles {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub parent_folder_id: Option<String>,
}

/// Moves a group file.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct MoveGroupFile {
    pub group_id: i64,
    pub file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub parent_folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub target_folder_id: Option<String>,
}

/// Renames a group file.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RenameGroupFile {
    pub group_id: i64,
    #[api_param(into)]
    pub file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[api_param(default)]
    pub parent_folder_id: Option<String>,
    #[api_param(into)]
    pub new_file_name: String,
}

/// Deletes a group file.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct DeleteGroupFile {
    pub group_id: i64,
    #[api_param(into)]
    pub file_id: String,
}

/// Creates a group folder.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot, response = String, field = "folder_id")]
pub struct CreateGroupFolder {
    pub group_id: i64,
    #[api_param(into)]
    pub folder_name: String,
}

/// Renames a group folder.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct RenameGroupFolder {
    pub group_id: i64,
    #[api_param(into)]
    pub folder_id: String,
    #[api_param(into)]
    pub new_folder_name: String,
}

/// Deletes a group folder.
#[derive(Debug, Serialize)]
#[api_payload(bot = MilkyBot)]
pub struct DeleteGroupFolder {
    pub group_id: i64,
    #[api_param(into)]
    pub folder_id: String,
}
