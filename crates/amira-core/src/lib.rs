//! # Amira Core
//!
//! The foundational types and interfaces for the Amira bot framework.
//!
//! This crate provides the fundamental abstractions that are not tied to any
//! specific framework design pattern. Higher-level constructs like dispatchers,
//! matchers, and convenience functions are in [`amira-framework`].
//!
//! ## Core Components
//!
//! ### Messages
//! - **ReceiveMessageSegment** / **SendMessageSegment**, **Message**:
//!   Cross-protocol message abstraction
//!
//! ### Transport
//! - **Capabilities**: Protocol-agnostic transport traits
//! - **TransportContext**: Capability discovery and registration
//! - **Connections**: Connection lifecycle and configuration
//!
//! ### Events
//! - **Event**: Type-erased event trait for protocol-specific types
//! - **EventType**: Event classification system
//! - **EventContext**: Wrapper for extracted event data
//!
//! ### Bots
//! - **Bot**: Protocol-agnostic bot trait
//!
//! ### Adapters
//! - **Adapter**: Protocol implementation trait
//! - **AdapterBridge**: Transport capability access for adapters

// Core modules
pub mod adapter;
pub mod bot;
pub mod bridge;
pub mod error;
pub mod event;
pub mod message;
pub mod transport;

// Re-export linkme so downstream crates don't need to add it as a direct
// dependency when using `register_capability`.
pub use linkme;

pub use bytes::Bytes;
pub use http::Method as HttpMethod;

// Re-export core types for public API
pub use bot::{ApiExecutor, ApiPayload, Bot};
pub use event::{EventRoot, EventType, EventView, Scene};
pub use message::{
    Message, ReceiveMessageSegment, RichText, RichTextSegment, SendMessageSegment, Sendable,
};
