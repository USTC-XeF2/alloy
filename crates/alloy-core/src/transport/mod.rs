//! Transport abstraction layer for the Alloy framework.
//!
//! This module provides the abstractions and types for managing bot connections
//! and transport capabilities across different protocols (HTTP, WebSocket, etc.).

pub mod capability;
pub mod config;
pub mod connection;

// Re-export commonly used types
pub use capability::{
    ConnectionHandler, HTTP_LISTEN_REGISTRY, HTTP_START_CLIENT_REGISTRY, HttpListenFn,
    HttpStartClientFn, TransportContext, WS_CONNECT_REGISTRY, WS_LISTEN_REGISTRY, WsConnectFn,
    WsListenFn,
};
pub use config::{HttpClientConfig, WsClientConfig};
pub use connection::{
    ConnectionHandle, ConnectionInfo, ConnectionKind, ListenerHandle, PostJsonFn,
};
