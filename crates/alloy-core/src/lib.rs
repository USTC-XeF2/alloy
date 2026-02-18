//! # Alloy Core
//!
//! The foundational types and interfaces for the Alloy bot framework.
//!
//! This crate provides the fundamental abstractions that are not tied to any
//! specific framework design pattern. Higher-level constructs like dispatchers,
//! matchers, and convenience functions are in [`alloy-framework`].
//!
//! ## Core Components
//!
//! ### Messages
//! - **MessageSegment**, **Message**: Cross-protocol message abstraction
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

// Re-export core types for public API
pub use adapter::{Adapter, AdapterContext, BoxedAdapter, ConfigurableAdapter, Dispatcher};
pub use bot::{Bot, BoxedBot};
pub use bridge::AdapterBridge;
pub use error::{
    AdapterError, AdapterResult, ApiError, ApiResult, TransportError, TransportResult,
};
pub use event::{AsText, BoxedEvent, Event, EventContext, EventType};
pub use message::{Message, MessageSegment, RichTextSegment};
pub use transport::{
    ClientConfig, ConnectionHandle, ConnectionHandler, ConnectionInfo, HttpClientCapability,
    HttpClientConfig, HttpServerCapability, HttpServerConfig, ListenerHandle, MessageHandler,
    RetryConfig, TransportConfig, TransportContext, TransportType, WsClientCapability,
    WsClientConfig, WsServerCapability, WsServerConfig,
};
