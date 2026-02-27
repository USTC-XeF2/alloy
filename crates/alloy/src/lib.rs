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
//! Alloy uses a plugin-based dispatch pipeline:
//!
//! ```text
//! ┌─────────────┐     ┌────────────┐     ┌──────────────────────────────────────────┐
//! │   Runtime   │────▶│ Dispatcher │────▶│ Plugin "echo"  (own task, own context)   │──▶ services
//! │  (Adapter)  │     │            │────▶│ Plugin "admin" (own task, own context)   │──▶ services
//! └─────────────┘     └────────────┘────▶│ Plugin ...     (own task, own context)   │──▶ services
//!                                        └──────────────────────────────────────────┘
//! ```
//!
//! - **Runtime**: Manages adapters, transports, and plugin lifecycle
//! - **Adapters**: Protocol implementations (OneBot, etc.)
//! - **Plugins**: Isolated event-handling units; each gets its own async task & context
//! - **Services**: Tower services (FilterLayer + HandlerService) within a plugin
//! - **Handlers**: User-defined async functions (Axum-style)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//! use alloy_adapter_onebot::{MessageEvent, OneBotAdapter};
//!
//! async fn echo(event: EventContext<MessageEvent>) -> anyhow::Result<String> {
//!     Ok(event.get_plain_text().to_string())
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AlloyRuntime::new();
//!     runtime.register_adapter::<OneBotAdapter>()?;
//!
//!     runtime.register_plugin(plugin! {
//!         name: "echo_plugin",
//!         services: [on_message().handler(echo)],
//!     }).await;
//!
//!     runtime.run().await;
//!     Ok(())
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
pub use alloy_transport as transport;

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

    // Plugin system - primary unit of event handling
    pub use alloy_framework::{
        define_plugin,
        plugin::{PluginDescriptor, ServiceInit, ServiceMeta},
    };

    // Event system - for building handlers
    pub use alloy_core::AsText;
    pub use alloy_framework::handler::{HandlerService, Layer, ServiceBuilderExt};

    // Extractors - for handler parameters
    pub use alloy_framework::extractor::{Bot, Event, FromContext, PluginConfig, ServiceRef};

    // Route convenience functions (from framework layer)
    pub use alloy_framework::routing::{on, on_event_type, on_message};

    // Structured command support (requires "command" feature)
    #[cfg(feature = "command")]
    pub use alloy_framework::command::{AtSegment, CommandArgs, ImageSegment, on_command};

    // Bot types - for interacting with bots in handlers
    pub use alloy_core::{Bot as __Bot, BoxedBot};

    // Core traits for custom implementations
    pub use alloy_core::{Event as __Event, Message, RichText, RichTextSegment};
}
