//! Transport abstraction layer for the Alloy framework.
//!
//! This module provides the abstractions and types for managing bot connections
//! and transport capabilities across different protocols (HTTP, WebSocket, etc.).

pub mod capability;
pub mod config;
pub mod connection;

// Re-export commonly used types
pub use capability::{ClientBotIdFn, ConnectionHandler, ServerBotIdFn, TransportContext};
pub use config::{
    HttpClientConfig, HttpServerConfig, SseClientConfig, WsClientConfig, WsServerConfig,
};
pub use connection::{ConnectionHandle, ConnectionInfo, HttpRequestFn, ListenerHandle, Sender};
