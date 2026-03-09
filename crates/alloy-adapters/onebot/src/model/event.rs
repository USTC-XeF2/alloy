//! OneBot v11 Event System — **parent-in-child** design.
//!
//! Each child event struct contains its parent via `#[serde(flatten)]`.
//! The `#[derive(BotEvent)]` macro auto-generates `Deref`/`DerefMut` so that
//! any child can transparently access all ancestor fields:
//!
//! ```text
//! PrivateMessageEvent  ──Deref──▶  MessageEvent  ──Deref──▶  OneBotEvent
//!   sub_type, temp_source           message_id, user_id, …    time, self_id
//! ```
//!
//! # Event Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id }                         ← root
//! ├── MessageEvent { message_id, user_id, message, … }  ← type = "message"
//! │   ├── PrivateMessageEvent { sub_type, temp_source }
//! │   └── GroupMessageEvent   { group_id, anonymous, sub_type }
//! ├── NoticeEvent {}                                     ← type = "notice"
//! │   ├── GroupUploadEvent, GroupAdminEvent, …
//! │   └── NotifyEvent { group_id, user_id }
//! │       ├── PokeEvent, LuckyKingEvent, HonorEvent
//! ├── RequestEvent {}                                    ← type = "request"
//! │   ├── FriendRequestEvent
//! │   └── GroupRequestEvent
//! └── MetaEvent {}                                       ← type = "meta"
//!     ├── LifecycleEvent
//!     └── HeartbeatEvent
//! ```
//!
//! # Parsing
//!
//! The adapter inspects `post_type`, `message_type`, etc. in the raw JSON
//! and constructs the **most specific** leaf event type. Because each child
//! embeds its parents via `#[serde(flatten)]`, all fields are deserialized
//! in a single pass.

use std::sync::Arc;

use alloy_core::BoxedEvent;
use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::message::OneBotMessage;
use crate::model::types::{Anonymous, Sender};

/// The root OneBot v11 event.
///
/// Contains common fields shared by **all** OneBot events.
/// Child events embed this via `#[serde(flatten)] parent: OneBotEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[root_event(platform = "onebot", segment_type = "crate::model::segment::Segment")]
pub struct OneBotEvent {
    /// Unix timestamp when the event occurred.
    pub time: i64,
    /// Bot's QQ ID.
    pub self_id: i64,
    /// Post type discriminator (e.g. "message", "notice", "request", "meta_event").
    pub post_type: String,
}

/// Message event with common fields.
///
/// `Deref` → [`OneBotEvent`], so `msg.time` and `msg.self_id` work directly.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message", type = "message")]
pub struct MessageEvent {
    /// Embedded parent fields (time, self_id, …).
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,

    /// Message type discriminator.
    pub message_type: String,
    /// Message ID.
    pub message_id: i32,
    /// Sender's user ID.
    #[event(user_id)]
    pub user_id: i64,
    /// Message content (array of segments).
    #[event(message)]
    #[serde(with = "super::message::serde_message")]
    pub message: OneBotMessage,
    /// Raw message string (CQ codes or plain text).
    pub raw_message: String,
    /// Font (usually 0).
    pub font: i32,
    /// Sender information.
    pub sender: Sender,
}

/// Private message event.
///
/// `Deref` chain: `PrivateMessageEvent` → [`MessageEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message.private", scene = "private")]
pub struct PrivateMessageEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MessageEvent,

    /// Sub-type ("friend", "group", "discuss", "other").
    pub sub_type: String,
}

/// Group message event.
///
/// `Deref` chain: `GroupMessageEvent` → [`MessageEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message.group", scene = "group")]
pub struct GroupMessageEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MessageEvent,

    /// Sub-type ("normal", "anonymous", "notice").
    pub sub_type: String,
    /// Group ID.
    #[event(group_id)]
    pub group_id: i64,
    /// Anonymous user info (if anonymous).
    #[serde(default)]
    pub anonymous: Option<Anonymous>,
}

/// Notice event base — matches any event with `post_type = "notice"`.
///
/// Use `EventContext<NoticeEvent>` to match **any** notice event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice", type = "notice")]
pub struct NoticeEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,

    pub notice_type: String,
}

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
#[event(name = "notice.group_upload", scene = "group")]
pub struct GroupUploadEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    #[event(group_id)]
    pub group_id: i64,
    #[event(user_id)]
    pub user_id: i64,
    pub file: UploadedFile,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_admin", scene = "group")]
pub struct GroupAdminEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    pub sub_type: String,
    #[event(group_id)]
    pub group_id: i64,
    #[event(user_id)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_decrease", scene = "group")]
pub struct GroupDecreaseEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    pub sub_type: String,
    #[event(group_id)]
    pub group_id: i64,
    pub operator_id: i64,
    #[event(user_id)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_increase", scene = "group")]
pub struct GroupIncreaseEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    pub sub_type: String,
    #[event(group_id)]
    pub group_id: i64,
    pub operator_id: i64,
    #[event(user_id)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_ban", scene = "group")]
pub struct GroupBanEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    pub sub_type: String,
    #[event(group_id)]
    pub group_id: i64,
    pub operator_id: i64,
    #[event(user_id)]
    pub user_id: i64,
    pub duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.friend_add", scene = "private")]
pub struct FriendAddEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    #[event(user_id)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.group_recall", scene = "group")]
pub struct GroupRecallEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    #[event(group_id)]
    pub group_id: i64,
    #[event(user_id)]
    pub user_id: i64,
    pub operator_id: i64,
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.friend_recall", scene = "private")]
pub struct FriendRecallEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    #[event(user_id)]
    pub user_id: i64,
    pub message_id: i64,
}

/// Notify event with common fields shared by poke / lucky_king / honor.
///
/// `Deref` → [`NoticeEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify")]
pub struct NotifyEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NoticeEvent,

    pub sub_type: String,
    #[event(user_id)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.poke")]
pub struct PokeEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,

    #[serde(default)]
    pub group_id: Option<i64>,
    pub target_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.lucky_king", scene = "group")]
pub struct LuckyKingEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,

    #[event(group_id)]
    pub group_id: i64,
    pub target_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "notice.notify.honor", scene = "group")]
pub struct HonorEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: NotifyEvent,

    #[event(group_id)]
    pub group_id: i64,
    pub honor_type: String,
}

/// Request event base — matches any event with `post_type = "request"`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request", type = "request")]
pub struct RequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,

    pub request_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.friend", scene = "private")]
pub struct FriendRequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: RequestEvent,

    #[event(user_id)]
    pub user_id: i64,
    pub comment: String,
    pub flag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.group", scene = "group")]
pub struct GroupRequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: RequestEvent,

    pub sub_type: String,
    #[event(group_id)]
    pub group_id: i64,
    #[event(user_id)]
    pub user_id: i64,
    pub comment: String,
    pub flag: String,
}

/// Meta event base — matches any event with `post_type = "meta_event"`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "meta_event", type = "meta")]
pub struct MetaEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "meta_event.lifecycle")]
pub struct LifecycleEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MetaEvent,
    /// Sub-type ("enable", "disable", "connect").
    pub sub_type: String,
}

/// Heartbeat status info.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatStatus {
    #[serde(default)]
    pub app_initialized: Option<bool>,
    #[serde(default)]
    pub app_enabled: Option<bool>,
    #[serde(default)]
    pub app_good: Option<bool>,
    #[serde(default)]
    pub online: Option<bool>,
    #[serde(default)]
    pub good: Option<bool>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "meta_event.heartbeat")]
pub struct HeartbeatEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MetaEvent,
    #[serde(default)]
    pub status: HeartbeatStatus,
    #[serde(default)]
    pub interval: i64,
}

/// Parses raw JSON into the most specific `BoxedEvent`.
///
/// The adapter calls this from `on_message`.
pub fn parse_onebot_event(raw: &str) -> serde_json::Result<BoxedEvent> {
    // Pre-parse to extract type discriminators
    let v: Value = serde_json::from_str(raw)?;
    let post_type = v.get("post_type").and_then(|v| v.as_str()).unwrap_or("");

    macro_rules! parse_event {
        ($ty:ty) => {
            Ok(Arc::new(serde_json::from_value::<$ty>(v)?))
        };
    }

    match post_type {
        "message" => {
            let msg_type = v.get("message_type").and_then(|v| v.as_str()).unwrap_or("");
            match msg_type {
                "private" => parse_event!(PrivateMessageEvent),
                "group" => parse_event!(GroupMessageEvent),
                _ => parse_event!(MessageEvent),
            }
        }
        "notice" => {
            let notice_type = v.get("notice_type").and_then(|v| v.as_str()).unwrap_or("");
            match notice_type {
                "group_upload" => parse_event!(GroupUploadEvent),
                "group_admin" => parse_event!(GroupAdminEvent),
                "group_decrease" => parse_event!(GroupDecreaseEvent),
                "group_increase" => parse_event!(GroupIncreaseEvent),
                "group_ban" => parse_event!(GroupBanEvent),
                "friend_add" => parse_event!(FriendAddEvent),
                "group_recall" => parse_event!(GroupRecallEvent),
                "friend_recall" => parse_event!(FriendRecallEvent),
                "notify" => {
                    let sub_type = v.get("sub_type").and_then(|v| v.as_str()).unwrap_or("");
                    match sub_type {
                        "poke" => parse_event!(PokeEvent),
                        "lucky_king" => parse_event!(LuckyKingEvent),
                        "honor" => parse_event!(HonorEvent),
                        _ => parse_event!(NotifyEvent),
                    }
                }
                _ => parse_event!(NoticeEvent),
            }
        }
        "request" => {
            let req_type = v.get("request_type").and_then(|v| v.as_str()).unwrap_or("");
            match req_type {
                "friend" => parse_event!(FriendRequestEvent),
                "group" => parse_event!(GroupRequestEvent),
                _ => parse_event!(RequestEvent),
            }
        }
        "meta_event" => {
            let meta_type = v
                .get("meta_event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match meta_type {
                "lifecycle" => parse_event!(LifecycleEvent),
                "heartbeat" => parse_event!(HeartbeatEvent),
                _ => parse_event!(MetaEvent),
            }
        }
        _ => parse_event!(OneBotEvent), // Unknown post_type — fall back to root event
    }
}
