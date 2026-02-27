//! # Alloy Framework
//!
//! High-level framework components for building bot applications.
//!
//! This layer provides:
//! - [`Plugin`] – the plugin abstraction that encapsulates handlers
//! - [`HandlerService`] – a simple tower `Service` that runs handlers
//! - [`FilterLayer`] – tower `Layer` for conditional dispatch
//! - Handler trait for Axum-style dependency injection
//! - Convenience route builders (`on_message`, `on_command`, etc.)
//! - [`define_plugin!`] – convenience macro for creating [`ServicePlugin`]s
//!
//! The framework layer is built on top of core types but adds higher-level
//! abstractions that aren't strictly necessary for the runtime.

pub mod context;
pub mod error;
pub mod extractor;
pub mod handler;
pub mod manager;
pub mod plugin;
pub mod routing;
pub mod service;

#[cfg(feature = "command")]
pub mod command;

pub use context::{AlloyContext, BaseContext, PluginContext};
pub use error::{EventSkipped, ExtractError, ExtractResult};
pub use extractor::FromContext;
pub use handler::{BoxedHandler, Handler, into_handler};
pub use manager::{PluginLoadState, PluginManager};
pub use plugin::{
    Plugin, PluginConfig, PluginDescriptor, PluginLoadContext, ServiceInit, ServiceMeta, ServiceRef,
};
pub use routing::{FilterServiceBuilder, on, on_event_type, on_message};
pub use service::{BoxedHandlerService, EventPredicate, HandlerService, ServiceBuilderExt};

pub use tower::{Layer, ServiceBuilder};

#[cfg(feature = "command")]
pub use command::{AtSegment, CommandArgs, ImageSegment, on_command};
