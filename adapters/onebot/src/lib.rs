//! # Amira Adapter for OneBot v11
//!
//! This crate provides an adapter for connecting the Amira bot framework
//! to OneBot v11 implementations.

mod adapter;
mod bot;

pub mod api;
pub mod config;
pub mod model;

pub use adapter::OneBotAdapter;
pub use bot::OneBotBot;

// Re-export model types for easier access.
pub use model::api::*;
pub use model::event::*;
pub use model::message::*;
pub use model::segment::*;
pub use model::types::*;
