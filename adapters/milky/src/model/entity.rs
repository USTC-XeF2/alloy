//! Milky protocol entity types (friends, groups, members, etc.).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Male,
    Female,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageSceneType {
    Friend,
    Group,
    Temp,
}

#[cfg(feature = "v1_2")]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionType {
    #[default]
    Face,
    Emoji,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FriendEntity {
    pub user_id: i64,
    pub nickname: String,
    pub sex: Sex,
    pub qid: String,
    pub remark: String,
    pub category: FriendCategoryEntity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FriendCategoryEntity {
    pub category_id: i32,
    pub category_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupEntity {
    pub group_id: i64,
    pub group_name: String,
    pub member_count: i32,
    pub max_member_count: i32,
    #[cfg(feature = "v1_2")]
    pub remark: String,
    #[cfg(feature = "v1_2")]
    pub created_time: i64,
    #[cfg(feature = "v1_2")]
    pub description: String,
    #[cfg(feature = "v1_2")]
    pub question: String,
    #[cfg(feature = "v1_2")]
    pub announcement: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupMemberEntity {
    pub user_id: i64,
    pub nickname: String,
    pub sex: Sex,
    pub group_id: i64,
    pub card: String,
    pub title: String,
    pub level: i32,
    pub role: GroupRole,
    pub join_time: i64,
    pub last_sent_time: i64,
    #[serde(default)]
    pub shut_up_end_time: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupAnnouncementEntity {
    pub group_id: i64,
    pub announcement_id: String,
    pub user_id: i64,
    pub time: i64,
    pub content: String,
    #[serde(default)]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupFileEntity {
    pub group_id: i64,
    pub file_id: String,
    pub file_name: String,
    pub parent_folder_id: String,
    pub file_size: i64,
    pub uploaded_time: i64,
    #[serde(default)]
    pub expire_time: Option<i64>,
    pub uploader_id: i64,
    pub downloaded_times: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupFolderEntity {
    pub group_id: i64,
    pub folder_id: String,
    pub parent_folder_id: String,
    pub folder_name: String,
    pub created_time: i64,
    pub last_modified_time: i64,
    pub creator_id: i64,
    pub file_count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FriendRequest {
    pub time: i64,
    pub initiator_id: i64,
    pub initiator_uid: String,
    pub target_user_id: i64,
    pub target_user_uid: String,
    pub state: String,
    pub comment: String,
    pub via: String,
    pub is_filtered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JoinState {
    Pending,
    Accepted,
    Rejected,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupNotificationType {
    JoinRequest {
        is_filtered: bool,
        initiator_id: i64,
        state: JoinState,
        #[serde(default)]
        operator_id: Option<i64>,
        comment: String,
    },
    AdminChange {
        target_user_id: i64,
        is_set: bool,
        operator_id: i64,
    },
    Kick {
        target_user_id: i64,
        operator_id: i64,
    },
    Quit {
        target_user_id: i64,
    },
    InvitedJoinRequest {
        initiator_id: i64,
        target_user_id: i64,
        state: JoinState,
        #[serde(default)]
        operator_id: Option<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupNotification {
    pub group_id: i64,
    pub notification_seq: i64,

    #[serde(flatten)]
    pub data: GroupNotificationType,
}
