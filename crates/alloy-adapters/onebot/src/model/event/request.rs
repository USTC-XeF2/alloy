//! Request Events — parent-in-child design.
//!
//! # Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id }
//! └── RequestEvent {}
//!     ├── FriendRequestEvent { user_id, comment, flag }
//!     └── GroupRequestEvent  { group_id, user_id, comment, flag, sub_type }
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};

use super::OneBotEvent;

// ============================================================================
// RequestEvent — marker for "any request"
// ============================================================================

/// Request event base — matches any event with `post_type = "request"`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request", type = "request")]
pub struct RequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
}

// ============================================================================
// FriendRequestEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.friend", type = "request")]
pub struct FriendRequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
    pub user_id: i64,
    #[serde(default)]
    pub comment: String,
    pub flag: String,
}

// ============================================================================
// GroupRequestEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(name = "request.group", type = "request")]
pub struct GroupRequestEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub comment: String,
    pub flag: String,
    pub sub_type: String,
}
