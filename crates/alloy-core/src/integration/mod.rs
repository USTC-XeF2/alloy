//! Integration layer - External system interfaces.
//!
//! This module contains interfaces for integrating with external systems:
//! - Adapter system for protocol implementations
//! - Bot management and lifecycle
//! - Capability-based transport system
//! - Transport configuration types

pub mod adapter;
pub mod bot;
pub mod capability;
pub mod transport;

#[allow(deprecated)]
pub use adapter::AdapterCapabilities;
pub use adapter::{Adapter, AdapterContext, AdapterFactory, AdapterRegistry, BoxedAdapter};

pub use bot::{
    ApiError, ApiResult, Bot, BotChannels, BotMessage, BoxedBot, RuntimeChannels, RuntimeMessage,
    create_bot_channels,
};

pub use capability::{
    // Context and management
    BotManager,
    // Connection handling
    BoxedConnectionHandler,
    // Handles
    ClientConfig,
    ConnectionHandle,
    ConnectionHandler,
    ConnectionInfo,
    // Transport capabilities
    HttpClientCapability,
    HttpServerCapability,
    ListenerHandle,
    MessageHandler,
    TransportContext,
    WsClientCapability,
    WsServerCapability,
};

pub use transport::{
    HttpClientConfig, HttpServerConfig, RetryConfig, TransportConfig, TransportType,
    WsClientConfig, WsServerConfig,
};
