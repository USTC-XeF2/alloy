//! OneBot-specific traits and types.
//!
//! This module provides traits specific to OneBot group and private message handling.

use async_trait::async_trait;

/// Represents the role or permission level of a group member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemberRole {
    /// Regular member with no special permissions.
    Member,
    /// Administrator with elevated permissions.
    Admin,
    /// Owner/creator of the group with full permissions.
    Owner,
}

impl Default for MemberRole {
    fn default() -> Self {
        Self::Member
    }
}

/// A trait for events that occur within a group context.
pub trait GroupEvent: Send + Sync {
    /// Returns the unique identifier of the group.
    fn get_group_id(&self) -> &str;

    /// Returns the role of the message sender in the group.
    fn get_sender_role(&self) -> MemberRole;

    /// Returns the user ID of the message sender.
    fn get_sender_id(&self) -> &str;

    /// Returns the display name of the sender in this group.
    fn get_sender_nickname(&self) -> Option<&str>;
}

/// A trait for group management operations.
#[async_trait]
pub trait GroupManagement: GroupEvent {
    /// Kicks a member from the group.
    async fn kick_member(&self, user_id: &str, reason: Option<&str>) -> anyhow::Result<()>;

    /// Mutes a member in the group.
    async fn mute_member(&self, user_id: &str, duration_secs: u64) -> anyhow::Result<()>;

    /// Sets the group's name.
    async fn set_group_name(&self, name: &str) -> anyhow::Result<()>;
}

/// A trait for private/direct message events.
pub trait PrivateEvent: Send + Sync {
    /// Returns the user ID of the other party in the conversation.
    fn get_user_id(&self) -> &str;

    /// Returns the nickname of the other party.
    fn get_user_nickname(&self) -> Option<&str>;
}
