//! Amira Runtime - Orchestration layer for the Amira bot framework.
//!
//! This crate provides:
//! - Runtime orchestration (`AmiraRuntime`)
//! - Automatic transport capability initialization
//! - Logging configuration
//!
//! Bots are managed by `amira_core::BotManager` within adapters,
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
//! use amira_runtime::{AmiraRuntime, RuntimeResult};
//!
//! #[tokio::main]
//! async fn main() -> RuntimeResult<()> {
//!     // Runtime automatically initializes transport capabilities
//!     let runtime = AmiraRuntime::new();
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
//! use amira_runtime::AmiraRuntime;
//!
//! #[tokio::main]
//! async fn main() {
//!     let runtime = AmiraRuntime::new();
//!     runtime.run().await.expect("Failed to run runtime");
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots are managed dynamically through `amira_core::BotManager`:
//! - Server transports: New connections automatically become bots
//! - Client transports: Connections auto-reconnect on disconnect
//! - Bots can join/leave at any time during runtime
//! - Bot queries and management via `BotManager` in adapters

pub mod config;
pub mod error;
pub mod handle;
pub mod runtime;

pub use config::{AmiraConfig, ConfigLoader};
pub use handle::BotHandle;
pub use runtime::AmiraRuntime;

#[cfg(feature = "logging")]
pub mod logging;
