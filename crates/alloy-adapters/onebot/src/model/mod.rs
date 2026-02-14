//! Data models for the OneBot v11 protocol.
//!
//! This module contains all the data structures used for communication
//! with OneBot v11 implementations.

pub mod api;
pub mod event;
pub mod message;
pub mod segment;
pub mod types;

pub use api::*;
pub use event::*;
pub use message::{OneBotMessage, parse_cq_string};
pub use segment::{
    AnonymousData, AtData, ContactData, DiceData, FaceData, ForwardData, ImageData, JsonData,
    LocationData, MusicData, NodeData, PokeData, RecordData, ReplyData, RpsData, Segment,
    ShakeData, ShareData, TextData, VideoData, XmlData,
};
pub use types::{Anonymous, Sender};
