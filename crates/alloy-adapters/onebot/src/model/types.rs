//! Common OneBot v11 types.
//!
//! This module defines shared types used across the OneBot v11 protocol,
//! such as sender information.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Male,
    Female,
    Unknown,
}

/// Private message sender information.
#[derive(Debug, Clone, Deserialize)]
pub struct PrivateSender {
    pub user_id: i64,
    pub nickname: String,
    #[serde(default)]
    pub sex: Option<Sex>,
    #[serde(default)]
    pub age: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupRole {
    Owner,
    Admin,
    Member,
}

/// Message sender information.
#[derive(Debug, Clone, Deserialize)]
pub struct Sender {
    /// Basic user information.
    #[serde(flatten)]
    base: PrivateSender,
    /// Group card (group nickname).
    #[serde(default)]
    pub card: Option<String>,
    /// Area.
    #[serde(default)]
    pub area: Option<String>,
    /// Membership level.
    #[serde(default)]
    pub level: Option<String>,
    /// Group role.
    #[serde(default)]
    pub role: Option<GroupRole>,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
}

impl std::ops::Deref for Sender {
    type Target = PrivateSender;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupRequestType {
    Add,
    Invite,
}

/// Status info.
#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    pub online: Option<bool>,
    pub good: bool,
}
