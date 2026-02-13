//! Notice Events — parent-in-child design.
//!
//! # Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id }
//! └── NoticeEvent {}                                   (marker for "any notice")
//!     ├── GroupUploadEvent  { group_id, user_id, file }
//!     ├── GroupAdminEvent   { group_id, user_id, sub_type }
//!     ├── GroupDecreaseEvent, GroupIncreaseEvent, GroupBanEvent
//!     ├── FriendAddEvent, GroupRecallEvent, FriendRecallEvent
//!     ├── GroupCardEvent, OfflineFileEvent, ClientStatusEvent, EssenceEvent
//!     └── NotifyEvent { group_id, user_id }
//!         ├── PokeEvent      { target_id }
//!         ├── LuckyKingEvent { target_id }
//!         └── HonorEvent     { honor_type }
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};

use super::OneBotEvent;

// ============================================================================
// NoticeEvent — marker for "any notice"
// ============================================================================

/// Notice event base — contains only the parent `OneBotEvent` fields.
///
/// Use `EventContext<NoticeEvent>` to match **any** notice event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice", type = "notice")]
pub struct NoticeEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
}

// ============================================================================
// Group Upload Event
// ============================================================================

/// Uploaded file info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub busid: i64,
}

/// Group file upload event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_upload", type = "notice")]
pub struct GroupUploadEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    pub file: UploadedFile,
}

// ============================================================================
// Group Admin Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_admin", type = "notice")]
pub struct GroupAdminEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    pub sub_type: String,
}

// ============================================================================
// Group Decrease Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_decrease", type = "notice")]
pub struct GroupDecreaseEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub sub_type: String,
}

// ============================================================================
// Group Increase Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_increase", type = "notice")]
pub struct GroupIncreaseEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub sub_type: String,
}

// ============================================================================
// Group Ban Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_ban", type = "notice")]
pub struct GroupBanEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub duration: i64,
    pub sub_type: String,
}

// ============================================================================
// Friend Add Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.friend_add", type = "notice")]
pub struct FriendAddEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub user_id: i64,
}

// ============================================================================
// Group Recall Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_recall", type = "notice")]
pub struct GroupRecallEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub message_id: i64,
}

// ============================================================================
// Friend Recall Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.friend_recall", type = "notice")]
pub struct FriendRecallEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub user_id: i64,
    pub message_id: i64,
}

// ============================================================================
// Group Card Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_card", type = "notice")]
pub struct GroupCardEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub card_new: Option<String>,
    #[serde(default)]
    pub card_old: Option<String>,
}

// ============================================================================
// Offline File Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineFile {
    pub name: String,
    pub size: i64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.offline_file", type = "notice")]
pub struct OfflineFileEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub user_id: i64,
    pub file: OfflineFile,
}

// ============================================================================
// Client Status Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    #[serde(default)]
    pub app_id: Option<i64>,
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub device_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.client_status", type = "notice")]
pub struct ClientStatusEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub client: Option<Device>,
}

// ============================================================================
// Essence Event
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.essence", type = "notice")]
pub struct EssenceEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    pub group_id: i64,
    pub sender_id: i64,
    pub operator_id: i64,
    pub message_id: i64,
    pub sub_type: String,
}

// ============================================================================
// NotifyEvent — common notify fields
// ============================================================================

/// Notify event with common fields shared by poke / lucky_king / honor.
///
/// `Deref` → [`NoticeEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify", type = "notice")]
pub struct NotifyEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,
    #[serde(default)]
    pub group_id: i64,
    pub user_id: i64,
}

// ============================================================================
// PokeEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.poke", type = "notice")]
pub struct PokeEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,
    pub target_id: i64,
}

// ============================================================================
// LuckyKingEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.lucky_king", type = "notice")]
pub struct LuckyKingEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,
    pub target_id: i64,
}

// ============================================================================
// HonorEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.honor", type = "notice")]
pub struct HonorEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,
    pub honor_type: String,
}
