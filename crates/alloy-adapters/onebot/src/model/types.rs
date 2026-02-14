//! Common OneBot v11 types.
//!
//! This module defines shared types used across the OneBot v11 protocol,
//! such as sender information and anonymous user data.

use serde::{Deserialize, Serialize};

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
