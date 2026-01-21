//! Framework layer - Core processing and routing.
//!
//! This module contains the framework's event processing pipeline:
//! - Handler trait and implementations for event processing (Axum-style)
//! - Matcher system for grouping handlers with check rules
//! - Parameter extraction for dependency injection
//! - Central dispatcher for event routing
//! - Tower Service integration for middleware support
//! - Matcher builder functions: [`on_message`], [`on_command`], [`on_notice`], [`on_request`], [`on_meta`]

pub mod dispatcher;
pub mod extractor;
pub mod handler;
pub mod matcher;
pub mod matcher_builders;

pub use dispatcher::Dispatcher;
pub use extractor::FromContext;
pub use handler::{
    BoxFuture, BoxedHandler, CanExtract, ErasedHandler, Handler, HandlerFn, into_handler,
};
pub use matcher::{CheckFn, Matcher, MatcherResponse};
pub use matcher_builders::{on_command, on_message, on_meta, on_notice, on_request};
