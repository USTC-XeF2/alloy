//! OneBot v11 Event System
//!
//! This module implements a Clap-like hierarchical event structure where:
//! - Each level extracts its common fields via `#[serde(flatten)]`
//! - Parent types contain common fields, child types contain specific fields
//!
//! # Event Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id, inner: OneBotEventKind }
//! └── OneBotEventKind (post_type dispatch)
//!     ├── Message(MessageEvent { message_id, user_id, message, ..., inner: MessageKind })
//!     │   └── MessageKind (message_type dispatch)
//!     │       ├── Private(PrivateMessageEvent { sub_type, temp_source })
//!     │       └── Group(GroupMessageEvent { group_id, anonymous, sub_type })
//!     ├── Notice(NoticeEvent { inner: NoticeKind })
//!     │   └── NoticeKind (notice_type dispatch)
//!     │       ├── GroupUpload, GroupAdmin, ...
//!     │       └── Notify(NotifyEvent { group_id, user_id, inner: NotifyKind })
//!     │           └── NotifyKind (sub_type dispatch)
//!     │               ├── Poke, LuckyKing, Honor
//!     ├── Request(RequestEvent { inner: RequestKind })
//!     │   └── RequestKind (request_type dispatch)
//!     │       ├── Friend(FriendRequestEvent)
//!     │       └── Group(GroupRequestEvent)
//!     └── MetaEvent(MetaEventEvent { inner: MetaEventKind })
//!         └── MetaEventKind (meta_event_type dispatch)
//!             ├── Lifecycle(LifecycleEvent)
//!             └── Heartbeat(HeartbeatEvent)
//! ```

pub mod message;
pub mod meta;
pub mod notice;
pub mod request;

use alloy_core::{Event, FromEvent};
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

/// The top-level OneBot v11 event.
///
/// Contains the common fields `time` and `self_id`, with `inner` dispatching
/// to specific event types based on `post_type`.
///
/// This struct also stores the raw JSON for event extraction at any hierarchy level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneBotEvent {
    /// Unix timestamp when the event occurred.
    pub time: i64,
    /// Bot's QQ ID.
    pub self_id: i64,
    /// The specific event kind, dispatched by post_type.
    #[serde(flatten)]
    pub inner: OneBotEventKind,
    /// Raw JSON string for extraction (not serialized).
    #[serde(skip)]
    raw: Option<Arc<str>>,
    /// Cached bot_id string (not serialized).
    #[serde(skip)]
    bot_id_str: Option<Arc<str>>,
}

impl OneBotEvent {
    /// Parses a JSON string into an OneBotEvent, preserving the raw JSON.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        let mut event: OneBotEvent = serde_json::from_str(json)?;
        event.raw = Some(Arc::from(json));
        event.bot_id_str = Some(Arc::from(event.self_id.to_string()));
        Ok(event)
    }

    /// Creates an OneBotEvent with raw JSON attached.
    pub fn with_raw(mut self, raw: impl Into<Arc<str>>) -> Self {
        self.raw = Some(raw.into());
        self.bot_id_str = Some(Arc::from(self.self_id.to_string()));
        self
    }
}

impl Event for OneBotEvent {
    fn event_name(&self) -> &'static str {
        self.inner.event_name()
    }

    fn platform(&self) -> &'static str {
        "onebot"
    }

    fn event_type(&self) -> alloy_core::EventType {
        self.inner.event_type()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn raw_json(&self) -> Option<&str> {
        self.raw.as_deref()
    }

    fn bot_id(&self) -> Option<&str> {
        self.bot_id_str.as_deref()
    }

    fn plain_text(&self) -> String {
        self.inner.plain_text()
    }
}

impl FromEvent for OneBotEvent {
    fn from_event(root: &dyn Event) -> Option<Self> {
        // Try raw JSON first
        if let Some(json) = root.raw_json()
            && let Ok(event) = serde_json::from_str::<OneBotEvent>(json)
        {
            // Re-attach the raw JSON
            return Some(event.with_raw(json.to_string()));
        }
        // Fallback: try direct downcast
        root.as_any().downcast_ref::<OneBotEvent>().cloned()
    }
}

/// Event kind dispatch based on `post_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "post_type")]
#[event(platform = "onebot", parent = "OneBotEvent")]
pub enum OneBotEventKind {
    /// Message events (private or group).
    #[serde(rename = "message")]
    Message(MessageEvent),
    /// Notice events (group changes, recalls, etc.).
    #[serde(rename = "notice")]
    Notice(NoticeEvent),
    /// Request events (friend/group requests).
    #[serde(rename = "request")]
    Request(RequestEvent),
    /// Meta events (lifecycle, heartbeat).
    #[serde(rename = "meta_event")]
    MetaEvent(MetaEventEvent),
}

impl OneBotEventKind {
    /// Extracts plain text from the event.
    pub fn plain_text(&self) -> String {
        match self {
            OneBotEventKind::Message(msg) => msg.plain_text(),
            _ => String::new(),
        }
    }
}
