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
//! ```rust,ignore
//! use alloy_runtime::AlloyRuntime;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Adapter auto-registers from config
//!     let runtime = AlloyRuntime::new();
//!     runtime.run().await
//! }
//! ```
//!
//! ## Programmatic Usage
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::{OneBotAdapter, OneBotConfig};
//!
//! // From config
//! let config: OneBotConfig = runtime.config().extract_adapter("onebot")?;
//! runtime.register_adapter(OneBotAdapter::from_config(config)).await;
//!
//! // Or build manually
//! let adapter = OneBotAdapter::builder()
//!     .ws_server("0.0.0.0:8080", "/ws")
//!     .ws_client("ws://localhost:6700/ws", None)
//!     .build();
//! runtime.register_adapter(adapter).await;
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

pub use adapter::{OneBotAdapter, OneBotAdapterBuilder};
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
    AnonymousData,
    AtData,
    ContactData,
    DiceData,
    FaceData,
    ForwardData,
    ImageData,
    JsonData,
    LocationData,
    MusicData,
    NodeData,
    PokeData,
    RecordData,
    ReplyData,
    RpsData,
    Segment,
    ShakeData,
    ShareData,
    TextData,
    VideoData,
    XmlData,
    // CQ code utilities
    escape_cq_text,
    escape_cq_value,
    unescape_cq_text,
    unescape_cq_value,
};

// Re-export message type and extension trait
pub use model::message::{OneBotMessage, OneBotMessageExt, parse_cq_string};

// Re-export event types
pub use model::event::{
    Anonymous,
    ClientStatusEvent,
    Device,
    EssenceEvent,
    FriendAddEvent,
    FriendRecallEvent,
    FriendRequestEvent,
    GroupAdminEvent,
    GroupBanEvent,
    GroupCardEvent,
    GroupDecreaseEvent,
    GroupIncreaseEvent,
    GroupMessageEvent,
    GroupRecallEvent,
    GroupRequestEvent,
    GroupUploadEvent,
    HeartbeatEvent,
    HonorEvent,
    LifecycleEvent,
    LuckyKingEvent,
    // Message types
    MessageEvent,
    // Meta types
    MetaEventEvent,
    // Notice types
    NoticeEvent,
    NotifyEvent,
    OfflineFile,
    OfflineFileEvent,
    // Root event
    OneBotEvent,
    PokeEvent,
    PrivateMessageEvent,
    // Request types
    RequestEvent,
    Sender,
    UploadedFile,
    // Parsing helper
    parse_onebot_event,
};
