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

#[cfg(feature = "command")]
pub mod command;
