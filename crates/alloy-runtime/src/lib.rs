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
//! use alloy_runtime::AlloyRuntime;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
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
//! use alloy_runtime::AlloyRuntime;
//! use alloy_core::TransportContext;
//! use alloy_transport::websocket::WsServerCapabilityImpl;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
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
pub use bot::{Bot, BotStatus};
pub use config::{AlloyConfig, BotConfig, ConfigError, ConfigLoader, ConfigResult, GlobalConfig};
pub use error::{RuntimeError, RuntimeResult};
pub use logging::{LoggingBuilder, SpanEvents};
pub use registry::{BotRegistry, RegistryStats};
pub use runtime::{AlloyRuntime, RuntimeStats};

// Re-export tracing for use by other crates
pub use tracing;
pub use tracing_subscriber;

/// Prelude module for convenient imports.
///
/// This provides all the commonly used logging macros:
/// - `trace!`, `debug!`, `info!`, `warn!`, `error!`
/// - `span`, `event`
/// - `instrument` attribute
/// - `Level` for span creation
pub mod prelude {
    pub use tracing::{Level, debug, error, info, instrument, span, trace, warn};
}
