//! # Alloy Adapter for OneBot v11
//!
//! This crate provides an adapter for connecting the Alloy bot framework
//! to OneBot v11 implementations.

mod adapter;
mod api_caller;
pub mod bot;
pub mod config;
pub mod model;

pub use adapter::OneBotAdapter;
pub use bot::OneBotBot;
pub use config::{
    ConnectionConfig, HttpClientConfig, HttpServerConfig, OneBotConfig, WsClientConfig,
    WsServerConfig,
};

// Re-export segment types
pub use model::segment::{
    AnonymousData, AtData, ContactData, DiceData, FaceData, ForwardData, ImageData, JsonData,
    LocationData, MusicData, NodeData, PokeData, RecordData, ReplyData, RpsData, Segment,
    ShakeData, ShareData, TextData, VideoData, XmlData,
};

// Re-export message type and extension trait
pub use model::message::{OneBotMessage, OneBotMessageExt};

// Re-export types
pub use model::types::{Anonymous, Sender};

// Re-export API response types
pub use model::api::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo, GroupMemberInfo, LoginInfo, Status,
    StrangerInfo, VersionInfo,
};

// Re-export event types
pub use model::event::{
    FriendAddEvent, FriendRecallEvent, FriendRequestEvent, GroupAdminEvent, GroupBanEvent,
    GroupDecreaseEvent, GroupIncreaseEvent, GroupMessageEvent, GroupRecallEvent, GroupRequestEvent,
    GroupUploadEvent, HeartbeatEvent, HonorEvent, LifecycleEvent, LuckyKingEvent, MessageEvent,
    MetaEvent, NoticeEvent, NotifyEvent, OneBotEvent, PokeEvent, PrivateMessageEvent, RequestEvent,
    UploadedFile,
};
