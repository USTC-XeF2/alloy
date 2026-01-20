//! Message Events
//!
//! # Hierarchy
//!
//! ```text
//! MessageEvent { message_id, user_id, message, raw_message, font, sender }
//! └── MessageKind (message_type dispatch)
//!     ├── Private(PrivateMessageEvent { sub_type, temp_source })
//!     └── Group(GroupMessageEvent { group_id, anonymous, sub_type })
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::OneBotEventKind;

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
// MessageEvent - Contains common message fields
// ============================================================================

/// Message event with common fields.
///
/// Contains fields shared by all message types:
/// - `message_id`, `user_id`, `message`, `raw_message`, `font`, `sender`
///
/// The `inner` field dispatches to `PrivateMessageEvent` or `GroupMessageEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message", platform = "onebot", parent = "OneBotEventKind")]
pub struct MessageEvent {
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
    /// The specific message kind.
    #[serde(flatten)]
    pub inner: MessageKind,
}

impl MessageEvent {
    /// Extracts plain text from the message.
    pub fn plain_text(&self) -> String {
        extract_plain_text(&self.message)
    }

    /// Returns the group_id if this is a group message.
    pub fn group_id(&self) -> Option<i64> {
        match &self.inner {
            MessageKind::Group(g) => Some(g.group_id),
            MessageKind::Private(_) => None,
        }
    }

    /// Returns true if this is a private message.
    pub fn is_private(&self) -> bool {
        matches!(self.inner, MessageKind::Private(_))
    }

    /// Returns true if this is a group message.
    pub fn is_group(&self) -> bool {
        matches!(self.inner, MessageKind::Group(_))
    }

    /// Returns the sub_type.
    pub fn sub_type(&self) -> &str {
        match &self.inner {
            MessageKind::Private(p) => &p.sub_type,
            MessageKind::Group(g) => &g.sub_type,
        }
    }

    /// Try to get as private message event.
    pub fn as_private(&self) -> Option<&PrivateMessageEvent> {
        match &self.inner {
            MessageKind::Private(p) => Some(p),
            _ => None,
        }
    }

    /// Try to get as group message event.
    pub fn as_group(&self) -> Option<&GroupMessageEvent> {
        match &self.inner {
            MessageKind::Group(g) => Some(g),
            _ => None,
        }
    }
}

/// Message kind dispatch based on `message_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "message_type")]
#[event(name = "message", platform = "onebot", parent = "MessageEvent")]
pub enum MessageKind {
    /// Private (direct) message.
    #[serde(rename = "private")]
    Private(PrivateMessageEvent),
    /// Group message.
    #[serde(rename = "group")]
    Group(GroupMessageEvent),
}

// ============================================================================
// PrivateMessageEvent - Private message specific fields
// ============================================================================

/// Private message specific fields.
///
/// Does NOT contain common fields - those are in `MessageEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message.private", platform = "onebot", parent = "MessageKind")]
pub struct PrivateMessageEvent {
    /// Sub-type ("friend", "group", "discuss", "other").
    #[serde(default)]
    pub sub_type: String,
    /// Temp source group (for temp conversations).
    #[serde(default)]
    pub temp_source: Option<i64>,
}

// ============================================================================
// GroupMessageEvent - Group message specific fields
// ============================================================================

/// Group message specific fields.
///
/// Does NOT contain common fields - those are in `MessageEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "message.group", platform = "onebot", parent = "MessageKind")]
pub struct GroupMessageEvent {
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
