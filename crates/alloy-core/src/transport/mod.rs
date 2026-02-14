//! Transport abstraction layer for the Alloy framework.
//!
//! This module provides the abstractions and types for managing bot connections
//! and transport capabilities across different protocols (HTTP, WebSocket, etc.).

pub mod capability;
pub mod config;
pub mod connection;

// Re-export commonly used types
pub use capability::{
    HttpClientCapability, HttpServerCapability, TransportContext, WsClientCapability,
    WsServerCapability,
};
pub use config::{
    HttpClientConfig, HttpServerConfig, RetryConfig, TransportConfig, TransportType,
    WsClientConfig, WsServerConfig,
};
pub use connection::{
    BoxedConnectionHandler, ClientConfig, ConnectionHandle, ConnectionHandler, ConnectionInfo,
    ListenerHandle, MessageHandler,
};
