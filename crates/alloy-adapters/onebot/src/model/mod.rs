//! Data models for the OneBot v11 protocol.
//!
//! This module contains all the data structures used for communication
//! with OneBot v11 implementations.

pub mod api;
pub mod event;
pub mod message;
pub mod segment;

pub use api::*;
pub use event::*;
pub use message::{OneBotMessage, parse_cq_string};
pub use segment::{
    // Segment type and data types
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
    escape_cq_text,
    escape_cq_value,
    unescape_cq_text,
    unescape_cq_value,
};
