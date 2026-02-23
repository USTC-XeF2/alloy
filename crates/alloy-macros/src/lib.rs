//! Procedural macros for the Alloy bot framework.
//!
//! This crate provides:
//!
//! - `#[derive(BotEvent)]` - Generates Event, Deref/DerefMut implementations
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
//! // Root event — no parent, defines platform + message type
//! #[derive(Clone, Serialize, Deserialize, BotEvent)]
//! #[root_event(platform = "onebot", message_type = "OneBotMessage")]
//! pub struct OneBotEvent {
//!     pub time: i64,
//!     pub self_id: i64,
//!     #[serde(skip)]
//!     #[event(raw_json)]
//!     raw: Option<Arc<str>>,
//! }
//!
//! // Child event — parent auto-detected from #[event(parent)] field
//! #[derive(Clone, Serialize, Deserialize, BotEvent)]
//! #[event(name = "message", type = "message")]
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
//! #[event(name = "message.private", type = "message")]
//! pub struct PrivateMessageEvent {
//!     #[event(parent)]
//!     #[serde(flatten)]
//!     pub parent: MessageEvent,
//!     pub sub_type: String,
//! }
//! ```

mod capability;
mod event;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives event-related implementations for structs.
///
/// Generates:
/// - `impl Event` — `event_name()`, `platform()`, `event_type()`, `as_any()`,
///   `downgrade_any()`, and optionally `raw_json()`, `get_message()`.
/// - `impl Deref<Target = Parent>` + `DerefMut` — when `#[event(parent)]` field exists.
///
/// # Root events: `#[root_event(…)]`
///
/// - `platform = "…"` — Platform name (also used as event name)
/// - `message_type = "…"` — Message type for all events of this platform
///
/// # Child events: `#[event(…)]`
///
/// - `name = "…"` — Event name suffix (auto-prefixed with `{platform}.`)
/// - `type = "message|notice|request|meta"` — EventType classification
///
/// # Field-level attributes `#[event(…)]`
///
/// - `parent` — Marks the parent field (type auto-detected)
/// - `raw_json` — `Option<Arc<str>>` field providing raw JSON
/// - `bot_id` — `Option<Arc<str>>` field providing bot ID
/// - `message` — Field implementing `Message` trait, used for `get_message()`
#[proc_macro_derive(BotEvent, attributes(event, root_event))]
pub fn derive_bot_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match event::derive_bot_event(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
/// Registers an async function as a transport capability implementation.
///
/// # Usage
///
/// Apply this attribute macro to an async function matching one of the four capability types:
///
/// ```rust,ignore
/// #[alloy_macros::register_capability(ws_client)]
/// pub async fn ws_connect(
///     url: String,
///     handler: Arc<dyn ConnectionHandler>,
///     config: ClientConfig,
/// ) -> TransportResult<ConnectionHandle> { ... }
///
/// #[alloy_macros::register_capability(ws_server)]
/// pub async fn ws_listen(
///     addr: String,
///     path: String,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<ListenerHandle> { ... }
///
/// #[alloy_macros::register_capability(http_client)]
/// pub async fn http_start_client(...) -> TransportResult<ConnectionHandle> { ... }
///
/// #[alloy_macros::register_capability(http_server)]
/// pub async fn http_listen(...) -> TransportResult<ListenerHandle> { ... }
/// ```
///
/// The macro leaves the decorated function unchanged and emits a `#[linkme::distributed_slice]`
/// static that wires the function into the capability registry in `alloy-core`.
///
/// The runtime calls [`TransportContext::collect_all()`] once at startup to
/// gather all registered capabilities.
///
/// The attribute argument must be one of: `ws_client`, `ws_server`, `http_client`, `http_server`.
#[proc_macro_attribute]
pub fn register_capability(attr: TokenStream, item: TokenStream) -> TokenStream {
    capability::register_capability(attr, item)
}
