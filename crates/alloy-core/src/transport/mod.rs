//! Transport abstraction layer for the Alloy framework.
//!
//! This module provides the abstractions and types for managing bot connections
//! and transport capabilities across different protocols (HTTP, WebSocket, etc.).

pub mod capability;
pub mod connection;

// Re-export commonly used types
pub use capability::{
    ConnectionHandler, HttpClientCapability, HttpServerCapability, TransportContext,
    WsClientCapability, WsServerCapability,
};
pub use connection::{
    ClientConfig, ConnectionHandle, ConnectionInfo, ConnectionKind, ListenerHandle, MessageHandler,
    PostJsonFn,
};
