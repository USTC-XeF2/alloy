//! # Alloy Adapter for OneBot v11
//!
//! This crate provides an adapter for connecting the Alloy bot framework
//! to OneBot v11 implementations.

mod adapter;
pub mod bot;
pub mod config;
pub mod model;

pub use adapter::OneBotAdapter;
pub use bot::OneBotBot;
pub use config::{
    ConnectionConfig, HttpClientConfig, HttpServerConfig, OneBotConfig, WsClientConfig,
    WsServerConfig,
};

// Re-export model types for easier access.
pub use model::api::*;
pub use model::event::*;
pub use model::message::*;
pub use model::segment::*;
pub use model::types::*;
