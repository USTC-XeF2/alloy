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
//! Alloy uses a tower-service-based dispatch pipeline:
//!
//! ```text
//! ┌─────────────┐     ┌────────────┐     ┌──────────────────────────────┐
//! │   Runtime   │────▶│ Dispatcher │────▶│  BoxedHandlerService         │──▶ handlers
//! │  (Adapter)  │     │            │────▶│  (FilterLayer + HandlerSvc)  │──▶ handlers
//! └─────────────┘     └────────────┘────▶│  ...                         │──▶ handlers
//!                                        └──────────────────────────────┘
//! ```
//!
//! - **Runtime**: Manages adapters, transports, and bot lifecycle
//! - **Adapters**: Protocol implementations (OneBot, etc.)
//! - **HandlerService**: Runs handlers; all checks are tower `Layer`s on top
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
//!     // Adapter is automatically configured from alloy.yaml
//!     runtime.register_adapter::<OneBotAdapter>()?;
//!
//!     // Register a service: FilterLayer gates HandlerService
//!     runtime.register_service(
//!         on_message().handler(echo).into()
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

pub use alloy_core as core;
pub use alloy_framework as framework;
pub use alloy_runtime as runtime;

// Optional: Re-export macros
#[cfg(feature = "macros")]
pub use alloy_macros as macros;

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
    pub use alloy_core::{AsText, EventContext};
    pub use alloy_framework::{Handler, HandlerService, Layer, ServiceBuilder, ServiceBuilderExt};

    // Route convenience functions (from framework layer)
    pub use alloy_framework::{on, on_event_type, on_message};

    // Structured command support (requires "command" feature)
    #[cfg(feature = "command")]
    pub use alloy_framework::{AtSegment, CommandArgs, ImageSegment, on_command};

    // Bot types - for interacting with bots in handlers
    pub use alloy_core::Bot;

    // Core traits for custom implementations
    pub use alloy_core::{Event, Message, RichTextSegment};
}
