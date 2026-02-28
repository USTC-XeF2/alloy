//! Handler, service, and routing system for the Alloy framework.
//!
//! This module contains the core handler system and extension middleware for
//! building event handling pipelines. It provides:
//!
//! - **Handler** ([`traits`]) – The core [`Handler`] trait that adapts functions
//!   with parameter injection (dependency injection), similar to Axum's system
//! - **Service** ([`service`]) – The `HandlerService` wrapper and tower
//!   `Layer` implementations for composing handlers
//!
//! # Architecture
//!
//! ## The Handler Trait
//!
//! The [`Handler`] trait is the foundation. It's automatically implemented for
//! async functions that:
//! - Accept 0-16 parameters that implement [`FromContext`]
//! - Return a type implementing [`HandlerResponse`]
//!
//! ```rust,ignore
//! use alloy_framework::*;
//! use alloy_core::event::message::PrivateMessage;
//!
//! // Simple handler - no parameters, no return
//! async fn on_any_event() {
//!     println!("Event occurred");
//! }
//!
//! // Handler with extractor - receives the typed event
//! async fn on_private_msg(event: Event<PrivateMessage>) {
//!     println!("Message: {}", event.get_plain_text());
//! }
//!
//! // Handler with return value - sends a message
//! async fn echo(event: Event<PrivateMessage>) -> String {
//!     event.get_plain_text()
//! }
//! ```
//!
//! ## Services and Layers
//!
//! The [`service::HandlerService`] wraps a handler and implements `tower::Service`.
//! Filtering and other cross-cutting concerns are expressed as standard tower layers
//! stacked on top via [`ServiceBuilder`]:
//!
//! ```text
//! on_message()              ← ServiceBuilder with FilterLayer pre-stacked
//!     .handler(my_handler)  ← HandlerService + FilterLayer + Identity
//! ```

pub mod builder;
pub mod service;
pub mod traits;

pub use builder::{BlockLayer, BlockService, EventPredicate, ServiceBuilderExt};
pub use service::{HandlerResponse, HandlerService};
pub use traits::FromCtxFn;

pub use tower::Layer;
