//! Extractor system for the Alloy framework.
//!
//! This module provides the [`FromContext`] trait and built-in implementations
//! that enable Alloy's parameter injection system. This allows handler functions
//! to declare what data they need, and the framework automatically provides it.
//!
//! # Core Concept
//!
//! The extractor system is built around the [`FromContext`] trait:
//! ```rust,ignore
//! #[async_trait]
//! pub trait FromContext: Sized + Send {
//!     async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self>;
//! }
//! ```
//!
//! Any type implementing this trait can be used as a handler parameter, and the
//! framework will attempt to extract it from the current context.
//!
//! # Error Handling
//!
//! If an extractor fails (returns `Err`), the handler is skipped with
//! [`EventSkipped`](crate::EventSkipped). Optional extractors with
//! [`Option<T>`] never fail.

pub mod bot;
pub mod core;
pub mod event;
pub mod plugin;

pub use bot::Bot;
pub use core::FromContext;
pub use event::Event;
pub use plugin::{PluginConfig, ServiceRef};
