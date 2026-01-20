//! Framework layer - Core processing and routing.
//!
//! This module contains the framework's event processing pipeline:
//! - Handler trait and implementations for event processing (Axum-style)
//! - Matcher system for grouping handlers with check rules
//! - Parameter extraction for dependency injection
//! - Central dispatcher for event routing
//! - Tower Service integration for middleware support

pub mod dispatcher;
pub mod extractor;
pub mod handler;
pub mod matcher;
pub mod service;

pub use dispatcher::Dispatcher;
pub use extractor::FromContext;
pub use handler::{
    BoxFuture, BoxedHandler, CanExtract, ErasedHandler, Handler, HandlerFn, into_handler,
};
pub use matcher::{CheckFn, Matcher, MatcherResponse};
pub use service::{AlloyError, BotCommand, ServiceFuture};
