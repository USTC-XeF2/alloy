//! # Alloy Core
//!
//! The core engine of the Alloy bot framework.
//!
//! This crate provides the fundamental building blocks for the Alloy framework,
//! including event handling, context management, and the central dispatcher.
//!
//! ## Architecture Layers
//!
//! Alloy Core is organized into three architectural layers:
//!
//! ### Foundation Layer
//!
//! Core abstractions and type system:
//! - **Event System**: Type-erased events with runtime downcasting ([`Event`], [`BoxedEvent`])
//! - **Context Management**: Event propagation and state ([`AlloyContext`])
//! - **Message Abstractions**: Cross-protocol communication ([`TextMessage`])
//!
//! ### Framework Layer
//!
//! Event processing and routing:
//! - **Handler System**: Event processing trait and implementations ([`Handler`], [`Outcome`])
//! - **Parameter Extraction**: Dependency injection via [`FromContext`]
//! - **Dispatcher**: Central event routing ([`Dispatcher`])
//! - **Tower Integration**: Middleware support ([`HandlerService`])
//!
//! ### Integration Layer
//!
//! External system interfaces:
//! - **Adapter System**: Protocol implementations ([`Adapter`])
//! - **Bot Management**: Bot lifecycle and state ([`Bot`])
//! - **Capability System**: Transport capabilities ([`TransportContext`])
//! - **Transport Config**: Configuration types for transports
//!
//! ## Hub-and-Spoke Architecture
//!
//! All events flow through the central [`Dispatcher`]:
//!
//! ```text
//! ┌─────────────┐     ┌────────────┐     ┌───────────┐
//! │   Adapter   │────▶│ Dispatcher │────▶│  Handler  │
//! │  (OneBot)   │     │   (Core)   │────▶│  Handler  │
//! └─────────────┘     └────────────┘────▶│  Handler  │
//!                                        └───────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use alloy_core::{Dispatcher, BoxedEvent, Event, Handler, AlloyContext, Outcome, BoxFuture};
//! use std::sync::Arc;
//!
//! // Define a custom event
//! struct MessageEvent {
//!     content: String,
//! }
//!
//! impl Event for MessageEvent {
//!     fn event_name(&self) -> &'static str {
//!         "message"
//!     }
//! }
//!
//! // Define a handler
//! struct EchoHandler;
//!
//! impl Handler for EchoHandler {
//!     fn check(&self, ctx: &AlloyContext) -> bool {
//!         ctx.event().is::<MessageEvent>()
//!     }
//!
//!     fn handle<'a>(&'a self, ctx: &'a AlloyContext) -> BoxFuture<'a, Outcome> {
//!         Box::pin(async move {
//!             if let Some(msg) = ctx.event().downcast::<MessageEvent>() {
//!                 println!("Received: {}", msg.content);
//!             }
//!             Outcome::Handled
//!         })
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut dispatcher = Dispatcher::new();
//!     dispatcher.register(EchoHandler);
//!
//!     let event = BoxedEvent::new(MessageEvent {
//!         content: "Hello, Alloy!".into(),
//!     });
//!
//!     dispatcher.dispatch(event).await;
//! }
//! ```

// Architectural layers
pub mod foundation;
pub mod framework;
pub mod integration;

// Re-export foundation types
pub use foundation::{
    AdapterError, AdapterResult, AlloyContext, BoxedEvent, Event, EventContext, ExtractError,
    ExtractResult, FromEvent, Message, MessageSegment, TransportError, TransportResult,
};

// Re-export framework types
pub use framework::{
    BoxFuture, BoxedHandler, CanExtract, Dispatcher, ErasedHandler, FromContext, Handler,
    HandlerFn, Matcher, MatcherResponse, into_handler,
};

// Re-export integration types
pub use integration::{
    Adapter, AdapterContext, ApiError, ApiResult, Bot, BotChannels, BotManager, BotMessage,
    BoxedAdapter, BoxedBot, BoxedConnectionHandler, ClientConfig, ConfigurableAdapter,
    ConnectionHandle, ConnectionHandler, ConnectionInfo, HttpClientCapability, HttpClientConfig,
    HttpServerCapability, HttpServerConfig, ListenerHandle, MessageHandler, RetryConfig,
    RuntimeChannels, RuntimeMessage, TransportConfig, TransportContext, TransportType,
    WsClientCapability, WsClientConfig, WsServerCapability, WsServerConfig, create_bot_channels,
};

/// Prelude for common imports.
pub mod prelude {
    pub use super::foundation::*;
    pub use super::framework::{
        BoxFuture, BoxedHandler, CanExtract, Dispatcher, FromContext, Handler, Matcher,
        MatcherResponse, into_handler,
    };
}
