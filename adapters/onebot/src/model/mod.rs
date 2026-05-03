//! Data models for the OneBot v11 protocol.
//!
//! This module contains all the data structures used for communication
//! with OneBot v11 implementations.

pub mod api;
pub mod event;
pub mod message;
pub mod segment;
pub mod types;

#[cfg(feature = "cqcode")]
mod cqcode;
