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
//! └── MetaEventEvent {}                                  ← type = "meta"
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

pub mod message;
pub mod meta;
pub mod notice;
pub mod request;

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use message::*;
pub use meta::*;
pub use notice::*;
pub use request::*;

// ============================================================================
// OneBotEvent (Root Level)
// ============================================================================

/// The root OneBot v11 event.
///
/// Contains common fields shared by **all** OneBot events.
/// Child events embed this via `#[serde(flatten)] parent: OneBotEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "onebot", platform = "onebot")]
pub struct OneBotEvent {
    /// Unix timestamp when the event occurred.
    pub time: i64,
    /// Bot's QQ ID.
    pub self_id: i64,
    /// Raw JSON string (not serialized — attached after initial parse).
    #[serde(skip)]
    #[event(raw_json)]
    raw: Option<Arc<str>>,
    /// Cached bot_id string (not serialized).
    #[serde(skip)]
    #[event(bot_id)]
    bot_id_str: Option<Arc<str>>,
}

impl OneBotEvent {
    /// Attaches raw JSON and caches bot_id.
    pub fn set_raw(&mut self, raw: &str) {
        self.raw = Some(Arc::from(raw));
        self.bot_id_str = Some(Arc::from(self.self_id.to_string()));
    }
}

// ============================================================================
// Adapter-level parse helper
// ============================================================================

/// Parses raw JSON into the most specific `BoxedEvent`.
///
/// The adapter calls this from `parse_event` / `on_message`.
pub fn parse_onebot_event(raw: &str) -> Result<alloy_core::BoxedEvent, serde_json::Error> {
    // Pre-parse to extract type discriminators
    let v: serde_json::Value = serde_json::from_str(raw)?;
    let post_type = v.get("post_type").and_then(|v| v.as_str()).unwrap_or("");

    macro_rules! attach_raw {
        ($ty:ty) => {{
            let mut event: $ty = serde_json::from_value(v)?;
            event.set_raw(raw);
            Ok(alloy_core::BoxedEvent::new(event))
        }};
    }

    match post_type {
        "message" => {
            let msg_type = v.get("message_type").and_then(|v| v.as_str()).unwrap_or("");
            match msg_type {
                "private" => attach_raw!(PrivateMessageEvent),
                "group" => attach_raw!(GroupMessageEvent),
                _ => attach_raw!(MessageEvent),
            }
        }
        "notice" => {
            let notice_type = v.get("notice_type").and_then(|v| v.as_str()).unwrap_or("");
            match notice_type {
                "group_upload" => attach_raw!(GroupUploadEvent),
                "group_admin" => attach_raw!(GroupAdminEvent),
                "group_decrease" => attach_raw!(GroupDecreaseEvent),
                "group_increase" => attach_raw!(GroupIncreaseEvent),
                "group_ban" => attach_raw!(GroupBanEvent),
                "friend_add" => attach_raw!(FriendAddEvent),
                "group_recall" => attach_raw!(GroupRecallEvent),
                "friend_recall" => attach_raw!(FriendRecallEvent),
                "group_card" => attach_raw!(GroupCardEvent),
                "offline_file" => attach_raw!(OfflineFileEvent),
                "client_status" => attach_raw!(ClientStatusEvent),
                "essence" => attach_raw!(EssenceEvent),
                "notify" => {
                    let sub_type = v.get("sub_type").and_then(|v| v.as_str()).unwrap_or("");
                    match sub_type {
                        "poke" => attach_raw!(PokeEvent),
                        "lucky_king" => attach_raw!(LuckyKingEvent),
                        "honor" => attach_raw!(HonorEvent),
                        _ => attach_raw!(NotifyEvent),
                    }
                }
                _ => attach_raw!(NoticeEvent),
            }
        }
        "request" => {
            let req_type = v.get("request_type").and_then(|v| v.as_str()).unwrap_or("");
            match req_type {
                "friend" => attach_raw!(FriendRequestEvent),
                "group" => attach_raw!(GroupRequestEvent),
                _ => attach_raw!(RequestEvent),
            }
        }
        "meta_event" => {
            let meta_type = v
                .get("meta_event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match meta_type {
                "lifecycle" => attach_raw!(LifecycleEvent),
                "heartbeat" => attach_raw!(HeartbeatEvent),
                _ => attach_raw!(MetaEventEvent),
            }
        }
        _ => {
            // Unknown post_type — fall back to root event
            let mut event: OneBotEvent = serde_json::from_value(v)?;
            event.set_raw(raw);
            Ok(alloy_core::BoxedEvent::new(event))
        }
    }
}
