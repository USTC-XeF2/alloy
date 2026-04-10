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

// Re-export core types for public API
pub use adapter::Adapter;
pub use bot::{Bot, BoxedBot};
pub use bridge::{AdapterBridge, BridgeRuntime, Dispatcher};
pub use error::{
    AdapterError, AdapterResult, ApiError, ApiResult, TransportError, TransportResult,
};
pub use event::{BoxedEvent, EventRoot, EventType, EventView, Scene};
pub use message::{
    Message, ReceiveMessageSegment, RichText, RichTextSegment, SendMessageSegment, Sendable,
};
pub use transport::{
    ClientBotIdFn, ConnectionHandle, ConnectionHandler, ConnectionInfo, HTTP_LISTEN_REGISTRY,
    HTTP_START_CLIENT_REGISTRY, HttpClientConfig, HttpListenFn, HttpServerConfig,
    HttpStartClientFn, ListenerHandle, PostJsonFn, SSE_CLIENT_REGISTRY, Sender, ServerBotIdFn,
    SseClientConfig, SseClientFn, TransportContext, WS_CONNECT_REGISTRY, WS_LISTEN_REGISTRY,
    WsClientConfig, WsConnectFn, WsListenFn, WsServerConfig,
};
