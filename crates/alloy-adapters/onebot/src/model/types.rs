//! Common OneBot v11 types.
//!
//! This module defines shared types used across the OneBot v11 protocol,
//! such as sender information.

use serde::{Deserialize, Serialize};

/// Private message sender information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSender {
    /// User ID.
    pub user_id: i64,
    /// Nickname.
    pub nickname: String,
    /// Gender ("male", "female", "unknown").
    #[serde(default)]
    pub sex: Option<String>,
    /// Age.
    #[serde(default)]
    pub age: Option<i32>,
}

/// Message sender information.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Group role ("owner", "admin", "member").
    #[serde(default)]
    pub role: Option<String>,
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

/// Status info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub online: Option<bool>,
    pub good: bool,
}
