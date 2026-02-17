//! # Alloy Adapter for OneBot v11
//!
//! This crate provides an adapter for connecting the Alloy bot framework
//! to OneBot v11 implementations.
//!
//! ## Configuration-Based Usage (Recommended)
//!
//! Configure in `alloy.yaml`:
//!
//! ```yaml
//! adapters:
//!   onebot:
//!     connections:
//!       - type: ws-server
//!         host: 0.0.0.0
//!         port: 8080
//!         path: /onebot/v11/ws
//!       - type: ws-client
//!         url: ws://127.0.0.1:6700/ws
//!         access_token: ${BOT_TOKEN:-}
//! ```
//!
//! ## Event Hierarchy
//!
//! ```text
//! OneBotEvent (implements Event trait)
//! ├── Message { Private, Group }
//! ├── Notice { GroupUpload, GroupAdmin, ... }
//! ├── Request { Friend, Group }
//! └── MetaEvent { Lifecycle, Heartbeat }
//! ```

mod adapter;
pub mod bot;
pub mod config;
pub mod model;

pub use adapter::OneBotAdapter;
pub use bot::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo, GroupMemberInfo, LoginInfo, OneBotBot,
    Status, StrangerInfo, VersionInfo,
};
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

// Re-export event types
pub use model::event::{
    ClientStatusEvent, Device, EssenceEvent, FriendAddEvent, FriendRecallEvent, FriendRequestEvent,
    GroupAdminEvent, GroupBanEvent, GroupCardEvent, GroupDecreaseEvent, GroupIncreaseEvent,
    GroupMessageEvent, GroupRecallEvent, GroupRequestEvent, GroupUploadEvent, HeartbeatEvent,
    HonorEvent, LifecycleEvent, LuckyKingEvent, MessageEvent, MetaEventEvent, NoticeEvent,
    NotifyEvent, OfflineFile, OfflineFileEvent, OneBotEvent, PokeEvent, PrivateMessageEvent,
    RequestEvent, UploadedFile,
};
