//! # Alloy Adapter for OneBot v11
//!
//! This crate provides an adapter for connecting the Alloy bot framework
//! to OneBot v11 implementations.
//!
//! ## Overview
//!
//! The OneBot protocol is a standard for QQ bots, and v11 is one of its
//! widely-used versions. This adapter handles:
//!
//! - Event parsing and dispatching
//! - Message serialization/deserialization
//! - Integration with alloy-runtime
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use alloy_runtime::AlloyRuntime;
//! use alloy_adapter_onebot::OneBotAdapter;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AlloyRuntime::load_config()?;
//!     runtime.register_adapter(OneBotAdapter::new()).await;
//!     runtime.run().await
//! }
//! ```
//!
//! ## Event Hierarchy
//!
//! Events are organized in a hierarchical structure:
//!
//! ```text
//! OneBotEvent (implements Event trait)
//! ├── Message { Private, Group }
//! ├── Notice { GroupUpload, GroupAdmin, ... , Notify { Poke, LuckyKing, Honor } }
//! ├── Request { Friend, Group }
//! └── MetaEvent { Lifecycle, Heartbeat }
//! ```
//!
//! ## Event Parsing
//!
//! Use [`OneBotEvent`] for automatic event type detection:
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotEvent;
//!
//! let event = OneBotEvent::parse(&json_string)?;
//! match event {
//!     OneBotEvent::Message(MessageEvent::Group(msg)) => {
//!         println!("Group: {}", msg.plain_text());
//!     }
//!     OneBotEvent::Message(MessageEvent::Private(msg)) => {
//!         println!("Private: {}", msg.plain_text());
//!     }
//!     _ => {}
//! }
//! ```
//!
//! ## Extractors
//!
//! The `extractors` module provides convenient types for extracting data:
//!
//! - [`extractors::Sender`]: Extract sender information
//! - [`extractors::GroupInfo`]: Extract group-specific info
//!
//! ## Bot API
//!
//! The [`OneBotBot`] provides strongly-typed methods for all OneBot v11 APIs:
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotBot;
//!
//! // In a handler
//! async fn my_handler(bot: BoxedBot, event: EventContext<MessageEvent>) {
//!     if let Some(onebot) = bot.as_any().downcast_ref::<OneBotBot>() {
//!         onebot.send_group_msg(12345, "Hello!", false).await.ok();
//!     }
//! }
//! ```

mod adapter;
pub mod bot;
pub mod extractors;
pub mod model;
pub mod traits;

pub use adapter::{OneBotAdapter, OneBotAdapterBuilder};
pub use bot::{
    Credentials, FriendInfo, GetMsgResponse, GroupInfo as GroupInfoResponse, GroupMemberInfo,
    LoginInfo, OneBotBot, Status, StrangerInfo, VersionInfo,
};
pub use extractors::{GroupInfo, Sender};

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

// Re-export message type
pub use model::message::{OneBotMessage, parse_cq_string};

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
    MessageKind,
    // Meta types
    MetaEventEvent,
    MetaEventKind,
    // Notice types
    NoticeEvent,
    NoticeKind,
    NotifyEvent,
    NotifyKind,
    OfflineFile,
    OfflineFileEvent,
    // Root event
    OneBotEvent,
    OneBotEventKind,
    PokeEvent,
    PrivateMessageEvent,
    // Request types
    RequestEvent,
    RequestKind,
    Sender as MessageSender,
    UploadedFile,
};

pub use traits::{GroupEvent, GroupManagement, MemberRole, PrivateEvent};

/// Legacy type alias for backward compatibility.
#[deprecated(since = "0.2.0", note = "Use `Segment` instead")]
pub type MessageSegment = Segment;
