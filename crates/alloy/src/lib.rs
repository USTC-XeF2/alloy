//! # Alloy
//!
//! A high-performance, highly decoupled, and type-safe bot framework for Rust.
//!
//! ## Overview
//!
//! Alloy is designed with the philosophy of "minimal core, pluggable capabilities,
//! type safety". It provides a framework for building bots that can work across
//! different protocols through a unified interface.
//!
//! ## Architecture
//!
//! Alloy uses a hub-and-spoke architecture with Matcher-based dispatch:
//!
//! ```text
//! ┌─────────────┐     ┌────────────┐     ┌───────────┐
//! │   Runtime   │────▶│ Dispatcher │────▶│  Matcher  │──▶ handlers
//! │  (Adapter)  │     │            │────▶│  Matcher  │──▶ handlers
//! └─────────────┘     └────────────┘────▶│  Matcher  │──▶ handlers
//!                                        └───────────┘
//! ```
//!
//! - **Runtime**: Manages adapters, transports, and bot lifecycle
//! - **Adapters**: Protocol implementations (OneBot, etc.)
//! - **Matchers**: Check rules + multiple handlers, control blocking
//! - **Handlers**: User-defined async functions (Axum-style)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//! use alloy_adapter_onebot::{MessageEvent, OneBotAdapter, OneBotConfig};
//!
//! async fn echo(event: EventContext<MessageEvent>) {
//!     if let Some(content) = event.get_plain_text().strip_prefix("/echo ") {
//!         info!("Echo: {}", content);
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AlloyRuntime::new();
//!     
//!     // Adapter is configured from alloy.yaml
//!     let config: OneBotConfig = runtime.config().extract_adapter("onebot")?;
//!     runtime.register_adapter(OneBotAdapter::from_config(config)).await;
//!     
//!     // Register a matcher with handlers
//!     runtime.register_matcher(
//!         Matcher::new()
//!             .on::<MessageEvent>()
//!             .handler(echo)
//!     ).await;
//!     
//!     runtime.run().await
//! }
//! ```
//!
//! ## Features
//!
//! - `macros`: Enable BotEvent derive macro (default)
//! - `adapter-onebot`: Enable OneBot v11 adapter
//! - `transport-ws`: Enable WebSocket transport
//! - `transport-http`: Enable HTTP transport
//!
//! ## Matcher System
//!
//! Matchers group handlers with a common check rule:
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//!
//! // Create a matcher that handles MessageEvent and blocks further matchers
//! let matcher = Matcher::new()
//!     .on::<MessageEvent>()  // Only handle message events
//!     .block(true)           // Block further matchers after this one
//!     .handler(echo_handler)
//!     .handler(log_handler);
//! ```

// Core types (includes Tower integration)
pub use alloy_core::*;

// Runtime
pub use alloy_runtime;

// Optional: Re-export macros
#[cfg(feature = "macros")]
pub use alloy_macros;

/// Prelude module for convenient imports.
///
/// This module provides all commonly used types for building bot applications:
///
/// ```rust,ignore
/// use alloy::prelude::*;
/// ```
pub mod prelude {
    // Runtime - main entry point
    pub use alloy_runtime::AlloyRuntime;

    // Event system - for building handlers
    pub use alloy_core::EventContext;
    pub use alloy_framework::{Handler, Matcher};

    // Matcher convenience functions (from framework layer)
    pub use alloy_framework::{on_message, on_meta, on_notice, on_request};

    // Structured command support (requires "command" feature)
    #[cfg(feature = "command")]
    pub use alloy_framework::{CommandArgs, on_command};

    // Bot types - for interacting with bots in handlers
    pub use alloy_core::Bot;

    // Core traits for custom implementations
    pub use alloy_core::{Event, Message};
}
