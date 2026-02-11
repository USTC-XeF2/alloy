//! Procedural macros for the Alloy bot framework.
//!
//! This crate provides:
//!
//! - `#[derive(BotEvent)]` - Generates event methods and FromEvent implementations
//!
//! # Handler System
//!
//! Note: The `#[handler]` macro has been removed. Handlers are now implemented
//! via blanket implementations for async functions, similar to Axum's approach.
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//!
//! // Just write async functions - no macro needed!
//! async fn echo_handler(event: EventContext<MessageEvent>) {
//!     println!("Message: {}", event.plain_text());
//! }
//!
//! // Use with Matcher
//! let matcher = Matcher::new()
//!     .on::<MessageEvent>()
//!     .handler(echo_handler);
//! ```
//!
//! # BotEvent Derive Macro
//!
//! The `BotEvent` derive macro generates methods and `FromEvent` implementations:
//!
//! ```rust,ignore
//! use alloy_macros::BotEvent;
//!
//! #[derive(Clone, BotEvent)]
//! #[event(platform = "onebot", parent = "MessageEvent")]
//! pub struct PrivateMessage {
//!     pub time: i64,
//!     pub self_id: i64,
//!     pub user_id: i64,
//!     pub raw_message: String,
//! }
//! ```

mod event;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives event-related implementations for structs and enums.
///
/// For **enums**, this generates:
/// - `event_name(&self) -> &'static str` - Returns the event name
/// - `self_id(&self) -> i64` - Returns the bot's ID (delegates to inner type)
/// - `time(&self) -> i64` - Returns the timestamp (delegates to inner type)
/// - `platform(&self) -> &'static str` - Returns the platform name
/// - `FromEvent` implementation for extraction
///
/// For **structs**, this generates:
/// - `event_name(&self) -> &'static str` - Returns the event name
/// - `platform(&self) -> &'static str` - Returns the platform name
/// - `FromEvent` implementation for extraction
///
/// # Attributes
///
/// - `#[event(platform = "...")]` - Set the platform name (default: "onebot")
/// - `#[event(name = "...")]` - Override the event name
/// - `#[event(parent = "...")]` - Parent event type for FromEvent chaining
///
/// # Example
///
/// ```rust,ignore
/// use alloy_macros::BotEvent;
///
/// // Enum with delegating methods
/// #[derive(Clone, BotEvent)]
/// #[event(platform = "onebot")]
/// pub enum MessageEvent {
///     #[serde(rename = "private")]
///     Private(PrivateMessage),
///     #[serde(rename = "group")]
///     Group(GroupMessage),
/// }
///
/// // Struct with parent for extraction chaining
/// #[derive(Clone, BotEvent)]
/// #[event(platform = "onebot", parent = "MessageEvent")]
/// pub struct PrivateMessage {
///     pub time: i64,
///     pub self_id: i64,
///     // ...
/// }
/// ```
#[proc_macro_derive(BotEvent, attributes(event))]
pub fn derive_bot_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match event::derive_bot_event(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
