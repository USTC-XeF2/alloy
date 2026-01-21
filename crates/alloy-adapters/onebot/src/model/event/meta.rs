//! Meta Events
//!
//! # Hierarchy
//!
//! ```text
//! MetaEventEvent { inner: MetaEventKind }
//! └── MetaEventKind (meta_event_type dispatch)
//!     ├── Lifecycle(LifecycleEvent)
//!     └── Heartbeat(HeartbeatEvent)
//! ```

use alloy_macros::BotEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::OneBotEventKind;

// ============================================================================
// MetaEventEvent - Container for meta events
// ============================================================================

/// Meta event container.
///
/// Dispatches to specific meta event types via `MetaEventKind`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "meta_event",
    platform = "onebot",
    parent = "OneBotEventKind",
    type = "meta"
)]
pub struct MetaEventEvent {
    /// The specific meta event kind.
    #[serde(flatten)]
    pub inner: MetaEventKind,
}

/// Meta event kind dispatch based on `meta_event_type`.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[serde(tag = "meta_event_type")]
#[event(
    name = "meta_event",
    platform = "onebot",
    parent = "MetaEventEvent",
    type = "meta"
)]
pub enum MetaEventKind {
    /// Lifecycle event.
    #[serde(rename = "lifecycle")]
    Lifecycle(LifecycleEvent),
    /// Heartbeat event.
    #[serde(rename = "heartbeat")]
    Heartbeat(HeartbeatEvent),
}

// ============================================================================
// LifecycleEvent
// ============================================================================

/// Lifecycle event (connect/disconnect).
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "meta_event.lifecycle",
    platform = "onebot",
    parent = "MetaEventKind"
)]
pub struct LifecycleEvent {
    /// Sub-type ("enable", "disable", "connect").
    pub sub_type: String,
}

// ============================================================================
// HeartbeatEvent
// ============================================================================

/// Heartbeat status info.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatStatus {
    /// App initialized.
    #[serde(default)]
    pub app_initialized: Option<bool>,
    /// App enabled.
    #[serde(default)]
    pub app_enabled: Option<bool>,
    /// App good.
    #[serde(default)]
    pub app_good: Option<bool>,
    /// Online status.
    #[serde(default)]
    pub online: Option<bool>,
    /// Good status.
    #[serde(default)]
    pub good: Option<bool>,
    /// Additional status fields.
    #[serde(flatten)]
    pub extra: Value,
}

/// Heartbeat event.
#[derive(Debug, Clone, Serialize, Deserialize, BotEvent)]
#[event(
    name = "meta_event.heartbeat",
    platform = "onebot",
    parent = "MetaEventKind"
)]
pub struct HeartbeatEvent {
    /// Status information.
    #[serde(default)]
    pub status: HeartbeatStatus,
    /// Heartbeat interval in milliseconds.
    #[serde(default)]
    pub interval: i64,
}
