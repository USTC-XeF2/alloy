//! OneBot v11 Event model based on EventView.

use alloy_core::Scene;
use alloy_macros::{event_data, event_root};
use serde::Deserialize;

use crate::model::message::OneBotMessage;
use crate::model::types::{PrivateSender, Sender, Status};

#[derive(Debug, Clone, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub busid: i64,
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
    #[event_view(name = MessageEvent, id = "message", type = Message)]
    Message {
        message_id: i32,
        #[event_field(message)]
        #[serde(with = "super::message::serde_message")]
        message: OneBotMessage,
        raw_message: String,
        font: i32,

        #[event_data]
        #[serde(flatten)]
        message_type: MessageType,
    },

    #[event_view(name = NoticeEvent, id = "notice", type = Notice)]
    Notice {
        #[event_data]
        #[serde(flatten)]
        notice_type: NoticeType,
    },

    #[event_view(name = RequestEvent, id = "request", type = Request)]
    Request {
        #[event_data]
        #[serde(flatten)]
        request_type: RequestType,
    },

    #[event_view(name = MetaEvent, id = "meta_event", type = Meta)]
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
            MessageType::Private { sender, .. } => sender,
            MessageType::Group { sender, .. } => sender,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = MessageEvent)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum MessageType {
    #[event_view(name = PrivateMessageEvent, id = "private", scene = Private)]
    Private {
        sub_type: String,
        #[event_field(user_id)]
        user_id: i64,
        sender: PrivateSender,
    },

    #[event_view(name = GroupMessageEvent, id = "group", scene = Group)]
    Group {
        sub_type: String,
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
pub enum NoticeType {
    #[event_view(name = GroupUploadEvent, id = "group_upload", scene = Group)]
    GroupUpload {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        file: UploadedFile,
    },

    #[event_view(name = GroupAdminEvent, id = "group_admin", scene = Group)]
    GroupAdmin {
        sub_type: String,
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(name = GroupDecreaseEvent, id = "group_decrease", scene = Group)]
    GroupDecrease {
        sub_type: String,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(name = GroupIncreaseEvent, id = "group_increase", scene = Group)]
    GroupIncrease {
        sub_type: String,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(name = GroupBanEvent, id = "group_ban", scene = Group)]
    GroupBan {
        sub_type: String,
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        duration: i64,
    },

    #[event_view(name = FriendAddEvent, id = "friend_add", scene = Private)]
    FriendAdd {
        #[event_field(user_id)]
        user_id: i64,
    },

    #[event_view(name = GroupRecallEvent, id = "group_recall", scene = Group)]
    GroupRecall {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        operator_id: i64,
        message_id: i64,
    },

    #[event_view(name = FriendRecallEvent, id = "friend_recall", scene = Private)]
    FriendRecall {
        #[event_field(user_id)]
        user_id: i64,
        message_id: i64,
    },

    #[event_view(name = NotifyEvent, id = "notify")]
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
    #[event_view(name = PokeEvent, id = "poke", scene_func = get_poke_scene)]
    Poke {
        #[serde(default)]
        group_id: Option<i64>,
        user_id: i64,
        target_id: i64,
    },

    #[event_view(name = LuckyKingEvent, id = "lucky_king", scene = Group)]
    LuckyKing {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        target_id: i64,
    },

    #[event_view(name = HonorEvent, id = "honor", scene = Group)]
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
pub enum RequestType {
    #[event_view(name = FriendRequestEvent, id = "friend")]
    Friend {
        user_id: i64,
        comment: String,
        flag: String,
    },

    #[event_view(name = GroupRequestEvent, id = "group")]
    Group {
        sub_type: String,
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
    #[event_view(name = LifecycleEvent, id = "lifecycle")]
    Lifecycle { sub_type: String },

    #[event_view(name = HeartbeatEvent, id = "heartbeat")]
    Heartbeat { status: Status, interval: i64 },
}
