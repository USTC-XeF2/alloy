//! Alloy Runtime - Orchestration layer for the Alloy bot framework.
//!
//! This crate provides:
//! - Bot instance management (`Bot`, `BotRegistry`)
//! - Runtime orchestration (`AlloyRuntime`)
//! - Automatic transport capability initialization
//! - Logging configuration
//!
//! # Automatic Transport Initialization
//!
//! The runtime automatically initializes all available transport capabilities
//! based on enabled cargo features:
//!
//! - `ws-client` (default): WebSocket client capability
//! - `ws-server`: WebSocket server capability
//! - `http-client`: HTTP client capability
//! - `http-server`: HTTP server capability
//!
//! ```ignore
//! use alloy_runtime::{AlloyRuntime, RuntimeResult};
//!
//! #[tokio::main]
//! async fn main() -> RuntimeResult<()> {
//!     // Runtime automatically initializes transport capabilities
//!     let runtime = AlloyRuntime::new();
//!     
//!     // Register adapters - they can discover and use available capabilities
//!     runtime.register_adapter(MyAdapter::new()).await;
//!     
//!     // Run until Ctrl+C
//!     runtime.run().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Manual Transport Configuration (Optional)
//!
//! You can also manually configure transport capabilities if needed:
//!
//! ```ignore
//! use alloy_runtime::{AlloyRuntime, RuntimeResult};
//! use alloy_core::TransportContext;
//! use alloy_transport::websocket::WsServerCapabilityImpl;
//!
//! #[tokio::main]
//! async fn main() -> RuntimeResult<()> {
//!     let runtime = AlloyRuntime::new();
//!     
//!     // Optionally override with custom transport context
//!     let ctx = TransportContext::new()
//!         .with_ws_server(WsServerCapabilityImpl::new());
//!     runtime.set_transport_context(ctx).await;
//!     
//!     runtime.run().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots are managed dynamically:
//! - Server transports: New connections automatically become bots
//! - Client transports: Connections auto-reconnect on disconnect
//! - Bots can join/leave at any time during runtime

pub mod bot;
pub mod config;
pub mod error;
pub mod logging;
pub mod registry;
pub mod runtime;

// Re-exports
pub use bot::{BotInstance, BotStatus};
pub use config::{
    AlloyConfig, ConfigError, ConfigLoader, ConfigResult, LogFormat, LogLevel, LogOutput,
    LoggingConfig, RuntimeConfig,
};
pub use error::{RuntimeError, RuntimeResult};
pub use logging::{LoggingBuilder, SpanEvents};
pub use registry::{BotRegistry, RegistryStats};
pub use runtime::{AlloyRuntime, RuntimeBuilder, RuntimeStats};
