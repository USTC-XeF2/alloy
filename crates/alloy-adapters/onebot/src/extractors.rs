//! Extractors for OneBot events.
//!
//! This module provides convenient extractors for extracting data from OneBot events.
//! These extractors implement `FromContext` and can be used directly in handler functions.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_macros::handler;
//! use alloy_adapter_onebot::extractors::Sender;
//!
//! #[handler]
//! async fn my_handler(sender: Sender, event: Arc<OneBotEvent>) -> Outcome {
//!     if let Some(msg) = event.as_message() {
//!         println!("Message from {}: {}", sender.nickname, msg.plain_text());
//!     }
//!     Outcome::Handled
//! }
//! ```

use alloy_core::{AlloyContext, FromContext};

use crate::model::event::{MessageEvent, MessageKind, OneBotEvent, OneBotEventKind};

/// Extracts sender information from a message event.
///
/// Contains the user ID, nickname, and context (group or private).
#[derive(Debug, Clone)]
pub struct Sender {
    /// The sender's user ID.
    pub user_id: i64,
    /// The sender's nickname.
    pub nickname: String,
    /// Whether this is from a group message.
    pub is_group: bool,
    /// The group ID, if this is a group message.
    pub group_id: Option<i64>,
}

impl Sender {
    /// Returns a display string for the sender's context.
    pub fn context_string(&self) -> String {
        if self.is_group {
            format!("group {}", self.group_id.unwrap_or(0))
        } else {
            "private".to_string()
        }
    }
}

/// Helper to extract Sender from MessageEvent
fn sender_from_message(msg: &MessageEvent) -> Sender {
    let (is_group, group_id) = match &msg.inner {
        MessageKind::Group(g) => (true, Some(g.group_id)),
        MessageKind::Private(_) => (false, None),
    };
    Sender {
        user_id: msg.user_id,
        nickname: msg.sender.nickname.clone().unwrap_or_default(),
        is_group,
        group_id,
    }
}

impl FromContext for Sender {
    type Error = ExtractorError;

    fn from_context(ctx: &AlloyContext) -> Result<Self, Self::Error> {
        // Try OneBotEvent (which now contains raw JSON)
        if let Some(event) = ctx.event().downcast_ref::<OneBotEvent>()
            && let OneBotEventKind::Message(msg) = &event.inner
        {
            return Ok(sender_from_message(msg));
        }
        Err(ExtractorError::TypeMismatch("Sender"))
    }
}

/// Extracts group information from a group message event.
///
/// This extractor only works with group messages.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    /// The group ID.
    pub group_id: i64,
    /// The message ID within the group.
    pub message_id: i32,
}

impl FromContext for GroupInfo {
    type Error = ExtractorError;

    fn from_context(ctx: &AlloyContext) -> Result<Self, Self::Error> {
        // Try OneBotEvent
        if let Some(event) = ctx.event().downcast_ref::<OneBotEvent>()
            && let OneBotEventKind::Message(msg) = &event.inner
            && let MessageKind::Group(g) = &msg.inner
        {
            return Ok(GroupInfo {
                group_id: g.group_id,
                message_id: msg.message_id,
            });
        }
        Err(ExtractorError::TypeMismatch("GroupInfo"))
    }
}

/// Error type for extractor failures.
#[derive(Debug, Clone)]
pub enum ExtractorError {
    /// The event type doesn't match what the extractor expects.
    TypeMismatch(&'static str),
    /// The required data is missing from the event.
    MissingData(&'static str),
}

impl std::fmt::Display for ExtractorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractorError::TypeMismatch(name) => {
                write!(f, "Cannot extract {name} from this event type")
            }
            ExtractorError::MissingData(name) => {
                write!(f, "Missing data for {name}")
            }
        }
    }
}

impl std::error::Error for ExtractorError {}
