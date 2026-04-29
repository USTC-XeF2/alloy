//! Milky protocol events based on EventView.

use alloy_core::{Message, Scene};
use alloy_macros::{event_data, event_root};
use serde::Deserialize;

#[cfg(feature = "v1_2")]
use crate::model::entity::ReactionType;
use crate::model::entity::{FriendEntity, GroupEntity, GroupMemberEntity, MessageSceneType};
use crate::model::message::IncomingSegment;

#[derive(Debug, Clone, Deserialize)]
#[event_root(platform = "milky")]
pub struct MilkyEvent {
    pub time: i64,
    pub self_id: i64,

    #[event_data]
    #[serde(flatten)]
    pub data: EventData,
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = MilkyEvent)]
#[serde(tag = "event_type", content = "data", rename_all = "snake_case")]
pub enum EventData {
    #[event_view(type = Meta)]
    BotOffline { reason: String },

    #[event_view(type = Message)]
    MessageReceive {
        peer_id: i64,
        message_seq: i64,
        sender_id: i64,
        #[serde(rename = "time")]
        send_time: i64,
        #[event_field(message)]
        segments: Message<IncomingSegment>,

        #[event_data]
        #[serde(flatten)]
        message_scene: Box<MessageScene>,
    },

    #[event_view(type = Notice, scene_func = get_event_scene)]
    MessageRecall {
        message_scene: MessageSceneType,
        peer_id: i64,
        message_seq: i64,
        sender_id: i64,
        operator_id: i64,
        display_suffix: String,
    },

    #[cfg(feature = "v1_2")]
    #[event_view(type = Notice, scene_func = get_event_scene)]
    PeerPinChange {
        message_scene: MessageSceneType,
        peer_id: i64,
        is_pinned: bool,
    },

    #[event_view(type = Request)]
    FriendRequest {
        initiator_id: i64,
        initiator_uid: String,
        comment: String,
        via: String,
    },

    #[event_view(type = Request)]
    GroupJoinRequest {
        group_id: i64,
        notification_seq: i64,
        is_filtered: bool,
        initiator_id: i64,
        comment: String,
    },

    #[event_view(type = Request)]
    GroupInvitedJoinRequest {
        group_id: i64,
        notification_seq: i64,
        initiator_id: i64,
        target_user_id: i64,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupInvitation {
        #[event_field(group_id)]
        group_id: i64,
        invitation_seq: i64,
        #[event_field(user_id)]
        initiator_id: i64,
        #[cfg(feature = "v1_2")]
        source_group_id: Option<i64>,
    },

    #[event_view(type = Notice, scene = Private)]
    FriendNudge {
        #[event_field(user_id)]
        user_id: i64,
        is_self_send: bool,
        is_self_receive: bool,
        display_action: String,
        display_suffix: String,
        display_action_img_url: String,
    },

    #[event_view(type = Notice, scene = Private)]
    FriendFileUpload {
        #[event_field(user_id)]
        user_id: i64,
        file_id: String,
        file_name: String,
        file_size: i64,
        file_hash: String,
        is_self: bool,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupAdminChange {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        #[cfg(feature = "v1_1")]
        operator_id: i64,
        is_set: bool,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupEssenceMessageChange {
        #[event_field(group_id)]
        group_id: i64,
        message_seq: i64,
        #[cfg(feature = "v1_1")]
        operator_id: i64,
        is_set: bool,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupMemberIncrease {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        #[serde(default)]
        operator_id: Option<i64>,
        #[serde(default)]
        invitor_id: Option<i64>,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupMemberDecrease {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        #[serde(default)]
        operator_id: Option<i64>,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupNameChange {
        #[event_field(group_id)]
        group_id: i64,
        new_group_name: String,
        operator_id: i64,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupMessageReaction {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        message_seq: i64,
        face_id: String,
        #[cfg(feature = "v1_2")]
        reaction_type: ReactionType,
        is_add: bool,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupMute {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        operator_id: i64,
        duration: i32,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupWholeMute {
        #[event_field(group_id)]
        group_id: i64,
        operator_id: i64,
        is_mute: bool,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupNudge {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        sender_id: i64,
        receiver_id: i64,
        display_action: String,
        display_suffix: String,
        display_action_img_url: String,
    },

    #[event_view(type = Notice, scene = Group)]
    GroupFileUpload {
        #[event_field(group_id)]
        group_id: i64,
        #[event_field(user_id)]
        user_id: i64,
        file_id: String,
        file_name: String,
        file_size: i64,
    },
}

fn get_event_scene(data: &EventData) -> Option<Scene> {
    match data {
        EventData::MessageRecall {
            message_scene,
            peer_id,
            sender_id,
            ..
        } => match message_scene {
            MessageSceneType::Friend => Some(Scene::Private {
                user_id: peer_id.to_string(),
            }),
            MessageSceneType::Group => Some(Scene::Group {
                group_id: peer_id.to_string(),
                user_id: Some(sender_id.to_string()),
            }),
            MessageSceneType::Temp => None,
        },
        #[cfg(feature = "v1_2")]
        EventData::PeerPinChange {
            message_scene,
            peer_id,
            ..
        } => match message_scene {
            MessageSceneType::Friend => Some(Scene::Private {
                user_id: peer_id.to_string(),
            }),
            MessageSceneType::Group => Some(Scene::Group {
                group_id: peer_id.to_string(),
                user_id: None,
            }),
            MessageSceneType::Temp => None,
        },
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[event_data(parent = MessageReceiveEvent)]
#[serde(tag = "message_scene", rename_all = "snake_case")]
pub enum MessageScene {
    #[event_view(name = FriendMessageEvent, scene_func = get_message_scene)]
    Friend { friend: FriendEntity },
    #[event_view(name = GroupMessageEvent, scene_func = get_message_scene)]
    Group {
        group: GroupEntity,
        group_member: GroupMemberEntity,
    },
    #[event_view(name = TempMessageEvent)]
    Temp { group: Option<GroupEntity> },
}

fn get_message_scene(message_scene: &MessageScene) -> Option<Scene> {
    match message_scene {
        MessageScene::Friend { friend } => Some(Scene::Private {
            user_id: friend.user_id.to_string(),
        }),
        MessageScene::Group {
            group,
            group_member,
        } => Some(Scene::Group {
            group_id: group.group_id.to_string(),
            user_id: Some(group_member.user_id.to_string()),
        }),
        MessageScene::Temp { .. } => None,
    }
}
