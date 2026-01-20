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
//! use alloy_adapter_onebot::{MessageEvent, OneBotAdapter};
//!
//! async fn echo(ctx: EventContext<MessageEvent>) {
//!     if let Some(content) = ctx.data().plain_text().strip_prefix("/echo ") {
//!         info!("Echo: {}", content);
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AlloyRuntime::new();
//!     runtime.register_adapter(OneBotAdapter::new()).await;
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
/// This module provides all commonly used types and traits in one import:
///
/// ```rust,ignore
/// use alloy::prelude::*;
/// ```
pub mod prelude {
    // Core types
    pub use alloy_core::{Adapter, BoxedAdapter};
    pub use alloy_core::{
        AlloyContext, BoxFuture, BoxedEvent, BoxedHandler, CanExtract, Dispatcher, Event,
        FromContext, Handler, HandlerFn, Matcher, MatcherExt, MatcherGroup, Message,
        MessageSegment, into_handler,
    };

    // Capability system
    pub use alloy_core::{
        AdapterContext, BotManager, ClientConfig, ConnectionHandle, ConnectionHandler,
        ConnectionInfo, HttpClientCapability, HttpServerCapability, ListenerHandle,
        TransportContext, WsClientCapability, WsServerCapability,
    };

    // Tower integration
    pub use alloy_core::{AlloyError, BotCommand, MatcherResponse, ServiceFuture};

    // Runtime types
    pub use alloy_runtime::{AlloyRuntime, BotStatus};

    // Logging
    pub use alloy_runtime::logging::{LoggingBuilder, SpanEvents};
}
