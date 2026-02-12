//! Procedural macros for the Alloy bot framework.
//!
//! This crate provides:
//!
//! - `#[derive(BotEvent)]` - Generates Event, FromEvent, Deref/DerefMut implementations
//!
//! # Parent-in-Child Event Design
//!
//! Events use a **parent-in-child** pattern where child events contain their
//! parent as a flattened field. The macro auto-generates `Deref`/`DerefMut`
//! so child events can transparently access all ancestor fields.
//!
//! ```rust,ignore
//! use alloy_macros::BotEvent;
//!
//! // Root event — no parent
//! #[derive(Clone, Serialize, Deserialize, BotEvent)]
//! #[event(name = "onebot", platform = "onebot")]
//! pub struct OneBotEvent {
//!     pub time: i64,
//!     pub self_id: i64,
//!     #[serde(skip)]
//!     #[event(raw_json)]
//!     raw: Option<Arc<str>>,
//! }
//!
//! // Child event — contains parent, auto Deref to OneBotEvent
//! #[derive(Clone, Serialize, Deserialize, BotEvent)]
//! #[event(
//!     name = "onebot.message", platform = "onebot",
//!     parent = "OneBotEvent", type = "message",
//!     validate(post_type = "message"),
//!     plain_text = "compute_plain_text",
//! )]
//! pub struct MessageEvent {
//!     #[event(parent)]
//!     #[serde(flatten)]
//!     pub parent: OneBotEvent,
//!     pub message_id: i32,
//!     pub user_id: i64,
//! }
//!
//! // Leaf event — Deref chain: PrivateMessageEvent → MessageEvent → OneBotEvent
//! #[derive(Clone, Serialize, Deserialize, BotEvent)]
//! #[event(
//!     name = "onebot.message.private", platform = "onebot",
//!     parent = "MessageEvent", type = "message",
//!     validate(post_type = "message", message_type = "private"),
//! )]
//! pub struct PrivateMessageEvent {
//!     #[event(parent)]
//!     #[serde(flatten)]
//!     pub parent: MessageEvent,
//!     pub sub_type: String,
//! }
//! ```

mod event;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives event-related implementations for structs.
///
/// Generates:
/// - `impl Event` — `event_name()`, `platform()`, `event_type()`, `as_any()`,
///   and optionally `raw_json()`, `bot_id()`, `plain_text()`.
/// - `impl FromEvent` — with optional JSON field validation.
/// - `impl Deref<Target = Parent>` + `DerefMut` — when `parent = "…"` is set.
///
/// # Struct-level attributes `#[event(…)]`
///
/// - `platform = "…"` — Platform name (default `"unknown"`)
/// - `name = "…"` — Full event name override
/// - `parent = "Type"` — Parent type (triggers Deref generation)
/// - `type = "message|notice|request|meta"` — EventType classification
/// - `plain_text = "method"` — Inherent method name for `plain_text()`
/// - `validate(key = "val", …)` — JSON field checks in `FromEvent`
///
/// # Field-level attributes `#[event(…)]`
///
/// - `parent` — Marks the parent field
/// - `raw_json` — `Option<Arc<str>>` field providing raw JSON
/// - `bot_id` — `Option<Arc<str>>` field providing bot ID
#[proc_macro_derive(BotEvent, attributes(event))]
pub fn derive_bot_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match event::derive_bot_event(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
