//! # Alloy Core
//!
//! The foundational types and interfaces for the Alloy bot framework.
//!
//! This crate provides the fundamental abstractions that are not tied to any
//! specific framework design pattern. Higher-level constructs like dispatchers,
//! matchers, and convenience functions are in [`alloy-framework`].
//!
//! ## Architecture Layers
//!
//! ### Foundation Layer
//!
//! Core abstractions and type system:
//! - **Event System**: Type-erased events with runtime downcasting ([`Event`], [`BoxedEvent`])
//! - **Context Management**: Event propagation and state ([`AlloyContext`])
//! - **Message Abstractions**: Cross-protocol communication ([`Message`], [`MessageSegment`])
//!
//! ### Integration Layer
//!
//! External system interfaces:
//! - **Adapter System**: Protocol implementations ([`Adapter`])
//! - **Bot Management**: Bot lifecycle and state ([`Bot`])
//! - **Capability System**: Transport capabilities ([`TransportContext`])
//! - **Transport Config**: Configuration types for transports

// Architectural layers
pub mod foundation;
pub mod integration;

// Re-export foundation types
pub use foundation::{
    AlloyContext, AsText, BoxedEvent, Event, EventContext, EventType, ExtractError, FromEvent,
    Message, MessageSegment, RichTextSegment,
};

// Re-export integration types
pub use integration::{
    Adapter,
    AdapterContext,
    ApiError,
    ApiResult,
    Bot,
    BotManager,
    BoxedAdapter,
    BoxedBot,
    // Capability types needed by transports
    BoxedConnectionHandler,
    ClientConfig,
    ConfigurableAdapter,
    ConnectionHandle,
    ConnectionHandler,
    ConnectionInfo,
    HttpClientCapability,
    HttpClientConfig,
    HttpServerCapability,
    HttpServerConfig,
    ListenerHandle,
    // Transport configuration types (used by runtime and adapters)
    RetryConfig,
    TransportConfig,
    TransportContext,
    TransportType,
    WsClientCapability,
    WsClientConfig,
    WsServerCapability,
    WsServerConfig,
};
