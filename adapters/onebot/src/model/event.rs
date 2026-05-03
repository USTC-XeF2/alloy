//! OneBot v11 Event model based on EventView.

use amira_core::Scene;
use amira_macros::{event_data, event_root};
use serde::Deserialize;

use crate::model::message::OneBotMessage;
use crate::model::types::{GroupRequestType, PrivateSender, Sender, Status};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateMessageType {
    Friend,
    Group,
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupMessageType {
    Normal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub busid: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminSetType {
    Set,
    Unset,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupDecreaseType {
    Leave,
    Kick,
    KickMe,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupIncreaseType {
    Approve,
    Invite,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBanType {
    Ban,
    LiftBan,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleType {
    Enable,
    Disable,
    Connect,
}

#[derive(Debug, Clone, Deserialize)]
#[event_root(platform = "onebot")]
pub struct OneBotEvent {
    pub time: i64,
    pub self_id: i64,

    #[event_data]
    #[serde(flatten)]
    pub data: EventData,
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = OneBotEvent)]
#[serde(tag = "post_type", rename_all = "snake_case")]
pub enum EventData {
    #[event_view(type = Message)]
    Message {
        message_id: i32,
        #[event_field(message)]
        #[cfg_attr(
            feature = "cqcode",
            serde(deserialize_with = "super::cqcode::deserialize")
        )]
        message: OneBotMessage,
        raw_message: String,
        font: i32,

        #[event_data]
        #[serde(flatten)]
        message_type: MessageEventType,
    },

    #[event_view(type = Notice)]
    Notice {
        #[event_data]
        #[serde(flatten)]
        notice_type: NoticeEventType,
    },

    #[event_view(type = Request)]
    Request {
        #[event_data]
        #[serde(flatten)]
        request_type: RequestEventType,
    },

    #[event_view(name = MetaEvent, type = Meta)]
    MetaEvent {
        #[event_data]
        #[serde(flatten)]
        meta_event_type: MetaEventType,
    },
}

impl MessageEvent {
    /// Get the sender information for this message event.
    pub fn sender(&self) -> &PrivateSender {
        match &self.message_type {
            MessageEventType::Private { sender, .. } => sender,
            MessageEventType::Group { sender, .. } => sender,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = MessageEvent)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum MessageEventType {
    #[event_view(name = PrivateMessageEvent, scene = Private)]
    Private {
        sub_type: PrivateMessageType,
        #[event_field(user_id)]
        user_id: i64,
        sender: PrivateSender,
    },

    #[event_view(name = GroupMessageEvent, scene = Group)]
    Group {
        sub_type: GroupMessageType,
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        sender: Sender,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = NoticeEvent)]
#[serde(tag = "notice_type", rename_all = "snake_case")]
pub enum NoticeEventType {
    #[event_view(scene = Group)]
    GroupUpload {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        file: UploadedFile,
    },

    #[event_view(scene = Group)]
    GroupAdmin {
        sub_type: AdminSetType,
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(scene = Group)]
    GroupDecrease {
        sub_type: GroupDecreaseType,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(scene = Group)]
    GroupIncrease {
        sub_type: GroupIncreaseType,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(scene = Group)]
    GroupBan {
        sub_type: GroupBanType,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        duration: i64,
    },

    #[event_view(scene = Private)]
    FriendAdd {
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(scene = Group)]
    GroupRecall {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        operator_id: i64,
        message_id: i64,
    },

    #[event_view(scene = Private)]
    FriendRecall {
        #[event_field(user_id)]
        user_id: i64,
        message_id: i64,
    },

    #[event_view]
    Notify {
        #[event_data]
        #[serde(flatten)]
        sub_type: NotifyType,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = NotifyEvent)]
#[serde(tag = "sub_type", rename_all = "snake_case")]
pub enum NotifyType {
    #[event_view(scene_func = get_poke_scene)]
    Poke {
        #[serde(default)]
        group_id: Option<i64>,
        user_id: i64,
        target_id: i64,
    },

    #[event_view(scene = Group)]
    LuckyKing {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        target_id: i64,
    },

    #[event_view(scene = Group)]
    Honor {
        #[event_field(group_id)]
        group_id: i64,
        honor_type: String,
        #[event_field(user_id)]
        user_id: i64,
    },
}

fn get_poke_scene(data: &NotifyType) -> Option<Scene> {
    match data {
        NotifyType::Poke {
            group_id, user_id, ..
        } => {
            if let Some(group_id) = group_id {
                Some(Scene::Group {
                    group_id: group_id.to_string(),
                    user_id: Some(user_id.to_string()),
                })
            } else {
                Some(Scene::Private {
                    user_id: user_id.to_string(),
                })
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = RequestEvent)]
#[serde(tag = "request_type", rename_all = "snake_case")]
pub enum RequestEventType {
    #[event_view(name = FriendRequestEvent)]
    Friend {
        user_id: i64,
        comment: String,
        flag: String,
    },

    #[event_view(name = GroupRequestEvent)]
    Group {
        sub_type: GroupRequestType,
        group_id: i64,
        user_id: i64,
        comment: String,
        flag: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = MetaEvent)]
#[serde(tag = "meta_event_type", rename_all = "snake_case")]
pub enum MetaEventType {
    #[event_view]
    Lifecycle { sub_type: LifecycleType },

    #[event_view]
    Heartbeat { status: Status, interval: i64 },
}
