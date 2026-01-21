//! Notice Events
//!
//! # Hierarchy
//!
//! ```text
//! NoticeEvent { inner: NoticeKind }
//! └── NoticeKind (notice_type dispatch)
//!     ├── GroupUpload(GroupUploadEvent)
//!     ├── GroupAdmin(GroupAdminEvent)
//!     ├── ... other notice types ...
//!     └── Notify(NotifyEvent { group_id, user_id, inner: NotifyKind })
//!         └── NotifyKind (sub_type dispatch)
//!             ├── Poke(PokeEvent)
//!             ├── LuckyKing(LuckyKingEvent)
//!             └── Honor(HonorEvent)
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};

use super::OneBotEventKind;

// ============================================================================
// NoticeEvent - Container for notice events
// ============================================================================

/// Notice event container.
///
/// Dispatches to specific notice types via `NoticeKind`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice",
    platform = "onebot",
    parent = "OneBotEventKind",
    type = "notice"
)]
pub struct NoticeEvent {
    /// The specific notice kind.
    #[serde(flatten)]
    pub inner: NoticeKind,
}

/// Notice kind dispatch based on `notice_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "notice_type")]
#[event(
    name = "notice",
    platform = "onebot",
    parent = "NoticeEvent",
    type = "notice"
)]
pub enum NoticeKind {
    /// Group file upload.
    #[serde(rename = "group_upload")]
    GroupUpload(GroupUploadEvent),
    /// Group admin change.
    #[serde(rename = "group_admin")]
    GroupAdmin(GroupAdminEvent),
    /// Group member decrease.
    #[serde(rename = "group_decrease")]
    GroupDecrease(GroupDecreaseEvent),
    /// Group member increase.
    #[serde(rename = "group_increase")]
    GroupIncrease(GroupIncreaseEvent),
    /// Group ban.
    #[serde(rename = "group_ban")]
    GroupBan(GroupBanEvent),
    /// Friend added.
    #[serde(rename = "friend_add")]
    FriendAdd(FriendAddEvent),
    /// Group message recall.
    #[serde(rename = "group_recall")]
    GroupRecall(GroupRecallEvent),
    /// Friend message recall.
    #[serde(rename = "friend_recall")]
    FriendRecall(FriendRecallEvent),
    /// Group card (nickname) changed.
    #[serde(rename = "group_card")]
    GroupCard(GroupCardEvent),
    /// Offline file received.
    #[serde(rename = "offline_file")]
    OfflineFile(OfflineFileEvent),
    /// Client status changed.
    #[serde(rename = "client_status")]
    ClientStatus(ClientStatusEvent),
    /// Essence message.
    #[serde(rename = "essence")]
    Essence(EssenceEvent),
    /// Notify event (poke, lucky_king, honor).
    #[serde(rename = "notify")]
    Notify(NotifyEvent),
}

// ============================================================================
// Group Upload Event
// ============================================================================

/// Uploaded file info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    /// File ID.
    pub id: String,
    /// File name.
    pub name: String,
    /// File size in bytes.
    pub size: i64,
    /// Download URL (busid).
    pub busid: i64,
}

/// Group file upload event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.group_upload",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct GroupUploadEvent {
    /// Group ID.
    pub group_id: i64,
    /// Uploader's user ID.
    pub user_id: i64,
    /// File information.
    pub file: UploadedFile,
}

// ============================================================================
// Group Admin Event
// ============================================================================

/// Group admin change event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.group_admin",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct GroupAdminEvent {
    /// Group ID.
    pub group_id: i64,
    /// User ID whose admin status changed.
    pub user_id: i64,
    /// Sub-type ("set" or "unset").
    pub sub_type: String,
}

// ============================================================================
// Group Decrease Event
// ============================================================================

/// Group member decrease event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.group_decrease",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct GroupDecreaseEvent {
    /// Group ID.
    pub group_id: i64,
    /// User ID who left or was kicked.
    pub user_id: i64,
    /// Operator user ID (who kicked, if applicable).
    #[serde(default)]
    pub operator_id: Option<i64>,
    /// Sub-type ("leave", "kick", "kick_me").
    pub sub_type: String,
}

// ============================================================================
// Group Increase Event
// ============================================================================

/// Group member increase event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.group_increase",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct GroupIncreaseEvent {
    /// Group ID.
    pub group_id: i64,
    /// User ID who joined.
    pub user_id: i64,
    /// Operator user ID (who approved/invited).
    #[serde(default)]
    pub operator_id: Option<i64>,
    /// Sub-type ("approve", "invite").
    pub sub_type: String,
}

// ============================================================================
// Group Ban Event
// ============================================================================

/// Group ban event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_ban", platform = "onebot", parent = "NoticeKind")]
pub struct GroupBanEvent {
    /// Group ID.
    pub group_id: i64,
    /// Banned user ID.
    pub user_id: i64,
    /// Operator user ID.
    #[serde(default)]
    pub operator_id: Option<i64>,
    /// Ban duration in seconds (0 = unban).
    pub duration: i64,
    /// Sub-type ("ban", "lift_ban").
    pub sub_type: String,
}

// ============================================================================
// Friend Add Event
// ============================================================================

/// Friend added event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.friend_add", platform = "onebot", parent = "NoticeKind")]
pub struct FriendAddEvent {
    /// New friend's user ID.
    pub user_id: i64,
}

// ============================================================================
// Group Recall Event
// ============================================================================

/// Group message recall event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.group_recall",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct GroupRecallEvent {
    /// Group ID.
    pub group_id: i64,
    /// Message author's user ID.
    pub user_id: i64,
    /// Operator user ID (who recalled).
    #[serde(default)]
    pub operator_id: Option<i64>,
    /// Recalled message ID.
    pub message_id: i64,
}

// ============================================================================
// Friend Recall Event
// ============================================================================

/// Friend message recall event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.friend_recall",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct FriendRecallEvent {
    /// Friend's user ID.
    pub user_id: i64,
    /// Recalled message ID.
    pub message_id: i64,
}

// ============================================================================
// Group Card Event
// ============================================================================

/// Group card (nickname) change event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_card", platform = "onebot", parent = "NoticeKind")]
pub struct GroupCardEvent {
    /// Group ID.
    pub group_id: i64,
    /// User ID whose card changed.
    pub user_id: i64,
    /// Old card (nickname).
    #[serde(default)]
    pub card_new: Option<String>,
    /// New card (nickname).
    #[serde(default)]
    pub card_old: Option<String>,
}

// ============================================================================
// Offline File Event
// ============================================================================

/// Offline file info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineFile {
    /// File name.
    pub name: String,
    /// File size in bytes.
    pub size: i64,
    /// Download URL.
    pub url: String,
}

/// Offline file received event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.offline_file",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct OfflineFileEvent {
    /// Sender's user ID.
    pub user_id: i64,
    /// File information.
    pub file: OfflineFile,
}

// ============================================================================
// Client Status Event
// ============================================================================

/// Device info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// App ID.
    #[serde(default)]
    pub app_id: Option<i64>,
    /// Device name.
    #[serde(default)]
    pub device_name: Option<String>,
    /// Device kind.
    #[serde(default)]
    pub device_kind: Option<String>,
}

/// Client status change event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.client_status",
    platform = "onebot",
    parent = "NoticeKind"
)]
pub struct ClientStatusEvent {
    /// Online status.
    #[serde(default)]
    pub online: bool,
    /// Client device info.
    #[serde(default)]
    pub client: Option<Device>,
}

// ============================================================================
// Essence Event
// ============================================================================

/// Essence message event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.essence", platform = "onebot", parent = "NoticeKind")]
pub struct EssenceEvent {
    /// Group ID.
    pub group_id: i64,
    /// Message sender ID.
    pub sender_id: i64,
    /// Operator ID.
    pub operator_id: i64,
    /// Message ID.
    pub message_id: i64,
    /// Sub-type ("add", "delete").
    pub sub_type: String,
}

// ============================================================================
// NotifyEvent - Contains common notify fields
// ============================================================================

/// Notify event with common fields.
///
/// Contains fields shared by all notify types:
/// - `group_id`, `user_id`
///
/// The `inner` field dispatches to specific notify events.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify", platform = "onebot", parent = "NoticeKind")]
pub struct NotifyEvent {
    /// Group ID (optional for some notify types).
    #[serde(default)]
    pub group_id: i64,
    /// User ID.
    pub user_id: i64,
    /// The specific notify kind.
    #[serde(flatten)]
    pub inner: NotifyKind,
}

/// Notify kind dispatch based on `sub_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "sub_type")]
#[event(name = "notice.notify", platform = "onebot", parent = "NotifyEvent")]
pub enum NotifyKind {
    /// Poke event.
    #[serde(rename = "poke")]
    Poke(PokeEvent),
    /// Lucky king (red packet) event.
    #[serde(rename = "lucky_king")]
    LuckyKing(LuckyKingEvent),
    /// Honor change event.
    #[serde(rename = "honor")]
    Honor(HonorEvent),
}

// ============================================================================
// Poke Event - Poke specific fields
// ============================================================================

/// Poke event specific fields.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.notify.poke",
    platform = "onebot",
    parent = "NotifyKind"
)]
pub struct PokeEvent {
    /// User who was poked.
    pub target_id: i64,
}

// ============================================================================
// Lucky King Event - Lucky king specific fields
// ============================================================================

/// Lucky king (red packet) event specific fields.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.notify.lucky_king",
    platform = "onebot",
    parent = "NotifyKind"
)]
pub struct LuckyKingEvent {
    /// User who got the lucky king.
    pub target_id: i64,
}

// ============================================================================
// Honor Event - Honor specific fields
// ============================================================================

/// Honor change event specific fields.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "notice.notify.honor",
    platform = "onebot",
    parent = "NotifyKind"
)]
pub struct HonorEvent {
    /// Honor type ("talkative", "performer", "emotion").
    pub honor_type: String,
}
