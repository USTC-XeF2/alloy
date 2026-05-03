//! # Amira
//!
//! A high-performance, highly decoupled, and type-safe bot framework for Rust.
//!
//! ## Overview
//!
//! Amira is designed with the philosophy of "minimal core, pluggable capabilities,
//! type safety". It provides a framework for building bots that can work across
//! different protocols through a unified interface.
//!
//! ## Architecture
//!
//! Amira uses a plugin-based dispatch pipeline:
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
//! use amira::prelude::*;
//! use amira_adapter_onebot::{MessageEvent, OneBotAdapter};
//!
//! async fn echo(event: Event<MessageEvent>) -> anyhow::Result<String> {
//!     Ok(event.plain_text())
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AmiraRuntime::new();
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

pub use amira_core as core;
pub use amira_framework as framework;
pub use amira_macros as macros;
pub use amira_transport as transport;

#[cfg(feature = "runtime")]
pub use amira_runtime as runtime;

/// Prelude module for convenient imports.
///
/// This module provides all commonly used types for building bot applications:
///
/// ```rust,ignore
/// use amira::prelude::*;
/// ```
pub mod prelude {
    // Runtime - main entry point
    #[cfg(feature = "runtime")]
    pub use amira_runtime::AmiraRuntime;

    // Plugin system - primary unit of event handling
    pub use amira_framework::plugin::PluginLoadContext;
    pub use amira_macros::define_plugin;

    // Event system - for building handlers
    pub use amira_core::{EventRoot, EventView};
    pub use amira_framework::handler::{HandlerService, Layer, ServiceBuilderExt};

    // Extractors - for handler parameters
    pub use amira_framework::context::{BoxedBot, HandlerContext};
    pub use amira_framework::extractor::{
        Bot, DefaultPluginState, Event, FromContext, PluginConfig, PluginState, ServiceRef,
    };

    // Route convenience functions (from framework layer)
    pub use amira_framework::routing::{on, on_event_type, on_message};

    // Structured command support (requires "command" feature)
    #[cfg(feature = "command")]
    pub use amira_framework::command::{
        AtSegment, CommandArgs, CommandMap, ImageSegment, help_command, on_command,
    };

    // Bot types - for interacting with bots in handlers
    pub use amira_core::{ApiExecutor, ApiPayload, Bot as __Bot};

    // Core traits for custom implementations
    pub use amira_core::{
        Message, ReceiveMessageSegment, RichText, RichTextSegment, Scene, SendMessageSegment,
    };
}
