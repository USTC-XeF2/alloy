//! # Alloy Framework
//!
//! High-level framework components for building bot applications.
//!
//! This layer provides:
//! - Matcher system for filtering and grouping handlers
//! - Handler trait for Axum-style dependency injection
//! - Convenience functions for common patterns (on_message, on_command, etc.)
//! - Clap-based command parsing system (with `command` feature)
//!
//! The framework layer is built on top of core types but adds higher-level
//! abstractions that aren't strictly necessary for the runtime.

pub mod context;
pub mod error;
pub mod extractor;
pub mod handler;
pub mod matcher;
pub mod matcher_builders;

#[cfg(feature = "command")]
pub mod command;

pub use context::AlloyContext;
pub use error::{ExtractError, ExtractResult};
pub use extractor::FromContext;
pub use handler::{
    BoxFuture, BoxedHandler, CanExtract, ErasedHandler, Handler, HandlerFn, into_handler,
};
pub use matcher::{CheckFn, Matcher, MatcherResponse};
pub use matcher_builders::{on_message, on_meta, on_notice, on_request};

#[cfg(feature = "command")]
pub use command::{AtSegment, CommandArgs, ImageSegment, on_command};
