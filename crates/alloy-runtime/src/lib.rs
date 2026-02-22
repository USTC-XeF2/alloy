//! Alloy Runtime - Orchestration layer for the Alloy bot framework.
//!
//! This crate provides:
//! - Runtime orchestration (`AlloyRuntime`)
//! - Automatic transport capability initialization
//! - Logging configuration
//!
//! Bots are managed by `alloy_core::BotManager` within adapters,
//! not directly by the runtime.
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
//!     runtime.register_adapter::<MyAdapter>().await?;
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
//! The transport context is automatically created with all available transport
//! capabilities based on enabled cargo features (ws-server, http-server, etc).
//!
//! ```ignore
//! use alloy_runtime::AlloyRuntime;
//!
//! #[tokio::main]
//! async fn main() {
//!     let runtime = AlloyRuntime::new();
//!     runtime.run().await.expect("Failed to run runtime");
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots are managed dynamically through `alloy_core::BotManager`:
//! - Server transports: New connections automatically become bots
//! - Client transports: Connections auto-reconnect on disconnect
//! - Bots can join/leave at any time during runtime
//! - Bot queries and management via `BotManager` in adapters

pub mod config;
pub mod error;
pub mod logging;
pub mod runtime;

pub use config::{AlloyConfig, ConfigLoader, LogFormat, LogLevel, LogOutput, LoggingConfig};
pub use error::{ConfigError, ConfigResult, RuntimeError, RuntimeResult};
pub use logging::{LoggingBuilder, SpanEvents};
pub use runtime::{AlloyRuntime, RuntimeBuilder};
