//! Meta Events — parent-in-child design.
//!
//! # Hierarchy
//!
//! ```text
//! OneBotEvent { time, self_id }
//! └── MetaEventEvent {}
//!     ├── LifecycleEvent { sub_type }
//!     └── HeartbeatEvent { status, interval }
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::OneBotEvent;

// ============================================================================
// MetaEventEvent — marker for "any meta event"
// ============================================================================

/// Meta event base — matches any event with `post_type = "meta_event"`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "onebot.meta_event",
    platform = "onebot",
    parent = "OneBotEvent",
    type = "meta"
)]
pub struct MetaEventEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
}

// ============================================================================
// LifecycleEvent
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "onebot.meta_event.lifecycle",
    platform = "onebot",
    parent = "OneBotEvent",
    type = "meta"
)]
pub struct LifecycleEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
    /// Sub-type ("enable", "disable", "connect").
    pub sub_type: String,
}

// ============================================================================
// HeartbeatEvent
// ============================================================================

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
#[event(
    name = "onebot.meta_event.heartbeat",
    platform = "onebot",
    parent = "OneBotEvent",
    type = "meta"
)]
pub struct HeartbeatEvent {
    #[event(parent)]
    #[serde(flatten)]
    pub parent: OneBotEvent,
    #[serde(default)]
    pub status: HeartbeatStatus,
    #[serde(default)]
    pub interval: i64,
}
