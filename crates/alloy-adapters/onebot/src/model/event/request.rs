//! Request Events
//!
//! # Hierarchy
//!
//! ```text
//! RequestEvent { inner: RequestKind }
//! └── RequestKind (request_type dispatch)
//!     ├── Friend(FriendRequestEvent)
//!     └── Group(GroupRequestEvent)
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};

use super::OneBotEventKind;

// ============================================================================
// RequestEvent - Container for request events
// ============================================================================

/// Request event container.
///
/// Dispatches to specific request types via `RequestKind`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "request",
    platform = "onebot",
    parent = "OneBotEventKind",
    type = "request"
)]
pub struct RequestEvent {
    /// The specific request kind.
    #[serde(flatten)]
    pub inner: RequestKind,
}

/// Request kind dispatch based on `request_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "request_type")]
#[event(
    name = "request",
    platform = "onebot",
    parent = "RequestEvent",
    type = "request"
)]
pub enum RequestKind {
    /// Friend request.
    #[serde(rename = "friend")]
    Friend(FriendRequestEvent),
    /// Group request (join or invite).
    #[serde(rename = "group")]
    Group(GroupRequestEvent),
}

// ============================================================================
// FriendRequestEvent
// ============================================================================

/// Friend request event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.friend", platform = "onebot", parent = "RequestKind")]
pub struct FriendRequestEvent {
    /// Requester's user ID.
    pub user_id: i64,
    /// Verification message.
    #[serde(default)]
    pub comment: String,
    /// Request flag (for approval/rejection).
    pub flag: String,
}

// ============================================================================
// GroupRequestEvent
// ============================================================================

/// Group request event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.group", platform = "onebot", parent = "RequestKind")]
pub struct GroupRequestEvent {
    /// Group ID.
    pub group_id: i64,
    /// Requester's user ID.
    pub user_id: i64,
    /// Verification message.
    #[serde(default)]
    pub comment: String,
    /// Request flag (for approval/rejection).
    pub flag: String,
    /// Sub-type ("add" or "invite").
    pub sub_type: String,
}
