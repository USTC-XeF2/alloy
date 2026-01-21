//! Event system for the Alloy framework.
//!
//! This module provides the core event infrastructure:
//!
//! - [`Event`] - Base trait for all events
//! - [`EventType`] - Event type classification (message, notice, request, meta)
//! - [`FromEvent`] - Trait for extracting typed events
//! - [`EventContext<T>`] - Wrapper providing access to extracted event data
//!
//! # Clap-like Event Extraction
//!
//! The event system supports a Clap-like pattern where events can be extracted
//! at any level of the hierarchy:
//!
//! ```rust,ignore
//! use alloy_core::{Event, FromEvent, EventContext};
//!
//! // Extract the most specific event type
//! async fn on_poke(ctx: EventContext<Poke>) {
//!     println!("Target: {}", ctx.target_id);
//! }
//!
//! // Extract an intermediate event type
//! async fn on_notice(ctx: EventContext<NoticeEvent>) {
//!     println!("Notice: {}", ctx.event_name());
//! }
//! ```

use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;

// ============================================================================
// Event Type Classification
// ============================================================================

/// Classification of event types.
///
/// This enum represents the high-level category of an event, which is useful
/// for filtering events in matchers without knowing the specific event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Message events (private messages, group messages, etc.)
    Message,
    /// Notice events (group changes, recalls, friend adds, etc.)
    Notice,
    /// Request events (friend requests, group join requests, etc.)
    Request,
    /// Meta events (lifecycle, heartbeat, etc.)
    Meta,
    /// Other/unknown event types
    Other,
}

impl EventType {
    /// Parses an event type from a string.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "message" => EventType::Message,
            "notice" => EventType::Notice,
            "request" => EventType::Request,
            "meta" | "meta_event" => EventType::Meta,
            _ => EventType::Other,
        }
    }
}

// ============================================================================
// Core Event Trait
// ============================================================================

/// The base trait for all events in the Alloy framework.
///
/// Events are type-erased using `dyn Event` and can be downcast to concrete
/// types using `as_any()`. Raw JSON is preserved to enable Clap-like extraction
/// at any hierarchy level via `FromEvent`.
///
/// # Derive Macro
///
/// Use `#[derive(BotEvent)]` to automatically implement common methods:
///
/// ```rust,ignore
/// #[derive(Clone, BotEvent)]
/// #[event(platform = "onebot")]
/// pub enum MessageEvent {
///     Private(PrivateMessage),
///     Group(GroupMessage),
/// }
/// ```
pub trait Event: Any + Send + Sync {
    /// Returns the human-readable name of this event type.
    fn event_name(&self) -> &'static str;

    /// Returns the platform/adapter name (e.g., "onebot", "discord").
    fn platform(&self) -> &'static str;

    /// Returns the high-level event type classification.
    ///
    /// This is used by matchers like `on_message()` to filter events
    /// without knowing the specific event type.
    fn event_type(&self) -> EventType {
        EventType::Other
    }

    /// Returns a reference to self as `Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns the raw JSON representation of this event, if available.
    ///
    /// This is essential for `FromEvent` to re-parse events at different
    /// hierarchy levels without losing information.
    fn raw_json(&self) -> Option<&str> {
        None
    }

    /// Returns the bot ID associated with this event, if available.
    ///
    /// This is used to route responses back to the correct bot instance.
    /// For OneBot, this would be `self_id` from the event.
    fn bot_id(&self) -> Option<&str> {
        None
    }

    /// Extracts plain text from the event, if applicable.
    ///
    /// For message events, this returns the message content.
    /// For other events, this returns an empty string by default.
    fn plain_text(&self) -> String {
        String::new()
    }
}

// ============================================================================
// Event Extraction
// ============================================================================

/// Trait for extracting typed events from a root event.
///
/// This enables Clap-like pattern matching where handlers can request
/// events at any level of the hierarchy. Implementations typically:
///
/// 1. Try to parse from `raw_json()` for maximum flexibility
/// 2. Fall back to downcasting for directly attached events
///
/// # Derive Macro
///
/// Use `#[derive(BotEvent)]` to auto-generate implementations:
///
/// ```rust,ignore
/// #[derive(Clone, Serialize, Deserialize, BotEvent)]
/// #[event(platform = "onebot", parent = "MessageEvent")]
/// pub struct PrivateMessage {
///     pub time: i64,
///     pub self_id: i64,
///     pub user_id: i64,
/// }
/// ```
pub trait FromEvent: Sized + Clone {
    /// Attempts to extract this event type from the root event.
    ///
    /// Returns `Some(Self)` if successful, `None` otherwise.
    fn from_event(root: &dyn Event) -> Option<Self>;
}

// ============================================================================
// Event Context
// ============================================================================

/// Context wrapper that provides access to extracted event data.
///
/// This is the primary way handlers receive events. Use `Deref` to access
/// fields directly on the wrapped type.
///
/// # Example
///
/// ```rust,ignore
/// #[handler]
/// async fn handler(ctx: EventContext<PrivateMessage>) -> Outcome {
///     // Access fields directly via Deref
///     println!("From: {} Message: {}", ctx.user_id, ctx.plain_text());
///     
///     // Access root event if needed
///     println!("Platform: {}", ctx.root.platform());
///     
///     Outcome::Handled
/// }
/// ```
#[derive(Clone)]
pub struct EventContext<T: Clone> {
    /// The extracted event data.
    data: T,
    /// Reference to the original root event.
    pub root: Arc<dyn Event>,
}

impl<T: Clone> EventContext<T> {
    /// Creates a new EventContext with the given data and root event.
    pub fn new(data: T, root: Arc<dyn Event>) -> Self {
        Self { data, root }
    }

    /// Returns a reference to the extracted data.
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Consumes the context and returns the extracted data.
    pub fn into_data(self) -> T {
        self.data
    }

    /// Returns the raw JSON of the root event, if available.
    pub fn raw_json(&self) -> Option<&str> {
        self.root.raw_json()
    }
}

impl<T: Clone> Deref for EventContext<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Clone + std::fmt::Debug> std::fmt::Debug for EventContext<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventContext")
            .field("data", &self.data)
            .field("root_event", &self.root.event_name())
            .finish()
    }
}

// ============================================================================
// Boxed Event
// ============================================================================

/// A type-erased container for events that supports runtime downcasting.
///
/// `BoxedEvent` wraps any type implementing [`Event`] in an `Arc`, allowing
/// it to be passed through the dispatcher without knowing its concrete type.
///
/// # Deref to Event Trait
///
/// `BoxedEvent` implements `Deref<Target = dyn Event>`, allowing you to call
/// any trait methods directly without using `.inner()`:
///
/// ```rust,ignore
/// let event: BoxedEvent = /* ... */;
/// let name = event.event_name();
/// let text = event.plain_text();
/// let typ = event.event_type();
/// ```
#[derive(Clone)]
pub struct BoxedEvent {
    inner: Arc<dyn Event>,
}

impl BoxedEvent {
    /// Creates a new `BoxedEvent` from any type implementing `Event`.
    pub fn new<E: Event + 'static>(event: E) -> Self {
        Self {
            inner: Arc::new(event),
        }
    }

    /// Returns the inner `Arc<dyn Event>`.
    pub fn inner(&self) -> &Arc<dyn Event> {
        &self.inner
    }

    /// Attempts to downcast to a concrete event type.
    pub fn downcast_ref<E: Event + 'static>(&self) -> Option<&E> {
        self.inner.as_any().downcast_ref()
    }

    /// Attempts to extract a typed event using `FromEvent`.
    pub fn extract<E: FromEvent>(&self) -> Option<EventContext<E>> {
        E::from_event(self.inner.as_ref()).map(|data| EventContext::new(data, self.inner.clone()))
    }
}

impl std::ops::Deref for BoxedEvent {
    type Target = dyn Event;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl std::fmt::Debug for BoxedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedEvent")
            .field("event_name", &self.event_name())
            .field("platform", &self.platform())
            .finish()
    }
}
