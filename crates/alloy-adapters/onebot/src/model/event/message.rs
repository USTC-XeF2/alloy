//! Message Events — parent-in-child design.
//!
//! # Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id }
//! └── MessageEvent { message_id, user_id, message, raw_message, font, sender }
//!     ├── PrivateMessageEvent { sub_type, temp_source }
//!     └── GroupMessageEvent   { group_id, anonymous, sub_type }
//! ```
//!
//! Each child `Deref`s to its parent, so `private_event.user_id` and
//! `private_event.time` both work transparently.

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::OneBotEvent;

// ============================================================================
// Shared Types
// ============================================================================

/// Message sender information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sender {
    /// User ID.
    #[serde(default)]
    pub user_id: Option<i64>,
    /// Nickname.
    #[serde(default)]
    pub nickname: Option<String>,
    /// Gender ("male", "female", "unknown").
    #[serde(default)]
    pub sex: Option<String>,
    /// Age.
    #[serde(default)]
    pub age: Option<i32>,
    /// Group card (group nickname).
    #[serde(default)]
    pub card: Option<String>,
    /// Area.
    #[serde(default)]
    pub area: Option<String>,
    /// Membership level.
    #[serde(default)]
    pub level: Option<String>,
    /// Group role ("owner", "admin", "member").
    #[serde(default)]
    pub role: Option<String>,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
}

/// Anonymous user information (for anonymous group messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anonymous {
    /// Anonymous user ID.
    pub id: i64,
    /// Anonymous user name.
    pub name: String,
    /// Flag for muting.
    pub flag: String,
}

// ============================================================================
// MessageEvent — common message fields + parent OneBotEvent
// ============================================================================

/// Message event with common fields.
///
/// `Deref` → [`OneBotEvent`], so `msg.time` and `msg.self_id` work directly.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "onebot.message",
    platform = "onebot",
    parent = "OneBotEvent",
    type = "message"
)]
pub struct MessageEvent {
    /// Embedded parent fields (time, self_id, …).
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,

    /// Message ID.
    pub message_id: i32,
    /// Sender's user ID.
    pub user_id: i64,
    /// Message content (array of segments).
    pub message: Value,
    /// Raw message string (CQ codes or plain text).
    pub raw_message: String,
    /// Font (usually 0).
    #[serde(default)]
    pub font: i32,
    /// Sender information.
    #[serde(default)]
    pub sender: Sender,
    /// Message type discriminator (kept for serde round-trip).
    #[serde(default)]
    pub message_type: String,
}

impl MessageEvent {
    /// Extracts plain text from the message segments.
    pub fn plain_text(&self) -> String {
        extract_plain_text(&self.message)
    }
}

// ============================================================================
// PrivateMessageEvent
// ============================================================================

/// Private message event.
///
/// `Deref` chain: `PrivateMessageEvent` → [`MessageEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "onebot.message.private",
    platform = "onebot",
    parent = "MessageEvent",
    type = "message"
)]
pub struct PrivateMessageEvent {
    /// Embedded parent fields (message_id, user_id, message, …, time, self_id).
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MessageEvent,

    /// Sub-type ("friend", "group", "discuss", "other").
    #[serde(default)]
    pub sub_type: String,
    /// Temp source group (for temp conversations).
    #[serde(default)]
    pub temp_source: Option<i64>,
}

// ============================================================================
// GroupMessageEvent
// ============================================================================

/// Group message event.
///
/// `Deref` chain: `GroupMessageEvent` → [`MessageEvent`] → [`OneBotEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "onebot.message.group",
    platform = "onebot",
    parent = "MessageEvent",
    type = "message"
)]
pub struct GroupMessageEvent {
    /// Embedded parent fields.
    #[event(parent)]
    #[serde(flatten)]
    pub parent: MessageEvent,

    /// Group ID.
    pub group_id: i64,
    /// Anonymous user info (if anonymous).
    #[serde(default)]
    pub anonymous: Option<Anonymous>,
    /// Sub-type ("normal", "anonymous", "notice").
    #[serde(default)]
    pub sub_type: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extracts plain text from message segments.
pub fn extract_plain_text(message: &Value) -> String {
    if let Value::Array(segments) = message {
        segments
            .iter()
            .filter_map(|seg| {
                if seg.get("type")?.as_str()? == "text" {
                    seg.get("data")?.get("text")?.as_str().map(String::from)
                } else {
                    None
                }
            })
            .collect::<String>()
    } else {
        String::new()
    }
}
