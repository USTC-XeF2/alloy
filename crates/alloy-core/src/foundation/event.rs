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
//! async fn on_poke(event: EventContext<Poke>) {
//!     println!("Target: {}", event.target_id);
//! }
//!
//! // Extract an intermediate event type
//! async fn on_notice(event: EventContext<NoticeEvent>) {
//!     println!("Notice: {}", event.event_name());
//! }
//! ```

use std::any::Any;
use std::ops::Deref;
use std::str::FromStr;
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

impl FromStr for EventType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "message" => EventType::Message,
            "notice" => EventType::Notice,
            "request" => EventType::Request,
            "meta" | "meta_event" => EventType::Meta,
            _ => EventType::Other,
        })
    }
}

// ============================================================================
// Text Extraction Trait
// ============================================================================

/// Trait for extracting text content from events.
///
/// This trait is automatically implemented for all types that implement [`Event`].
/// It provides an object-safe way to extract both plain text and rich text content
/// from events, even when accessed through a trait object.
///
/// # Type Safety
///
/// While `AsText` itself is object-safe and can be used with `dyn AsText`,
/// the blanket implementation `impl<E: Event> AsText for E` leverages the fact
/// that concrete types are `Sized` to safely call `get_message()`. This means:
///
/// - Direct calls on concrete types: Always works
/// - Calls through `&dyn AsText`: Always works
/// - Calls through `&dyn Event`: Not available (use downcasting or trait casting if needed)
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::{Event, AsText};
///
/// fn process_events(events: Vec<Box<dyn AsText>>) {
///     for event in events {
///         println!("Plain: {}", event.get_plain_text());
///         for seg in event.get_rich_text() {
///             println!("  {:?}", seg);
///         }
///     }
/// }
/// ```
pub trait AsText: Send + Sync {
    /// Extracts plain text from the event's message.
    ///
    /// For message events, this returns the concatenated text content of all
    /// text segments. For non-message events, returns an empty string.
    fn get_plain_text(&self) -> String;

    /// Extracts rich text segments from the event's message.
    ///
    /// Returns a vector of [`RichTextSegment`] representing the full message
    /// content including images, mentions, and text. For non-message events,
    /// returns an empty vector.
    fn get_rich_text(&self) -> Vec<super::message::RichTextSegment>;
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
/// All events automatically implement [`AsText`], which provides the
/// `get_plain_text()` and `get_rich_text()` methods in a type-safe way.
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
pub trait Event: AsText + Any + Send + Sync {
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

    /// The segment type used by messages in this event's platform.
    ///
    /// For all events under the same adapter/platform, this should be the same type
    /// (e.g., all OneBot events use `onebot::Segment`). The root event specifies the
    /// segment type, and child events inherit it.
    ///
    /// Messages are represented as `Message<Self::Segment>`.
    ///
    /// This is gated by `Self: Sized` so that `dyn Event` remains object-safe.
    type Segment: super::message::MessageSegment
    where
        Self: Sized;

    /// Returns a reference to the message contained in this event.
    ///
    /// Only available on concrete (sized) event types, not through `dyn Event`.
    /// For message events, adapters should return the concrete message.
    /// For non-message events, returns an empty message by default.
    fn get_message(&self) -> &super::message::Message<Self::Segment>
    where
        Self: Sized,
    {
        // This should be overridden by the macro for events with message fields.
        // For events without message fields, this would need a static empty instance.
        // The macro will generate proper implementations.
        panic!("get_message() called on event without message field - macro should override this")
    }
}

// ============================================================================
// AsText Blanket Implementation
// ============================================================================

/// Automatic implementation of [`AsText`] for all [`Event`] types.
///
/// This blanket implementation safely leverages the `Sized` bound on concrete
/// types to call `get_message()` and extract text. While both `Event`
/// and `AsText` are object-safe on their own, this implementation ensures
/// that whenever you use `&dyn AsText`, you get the correct behavior.
impl<E: Event> AsText for E {
    fn get_plain_text(&self) -> String {
        self.get_message().extract_plain_text()
    }

    fn get_rich_text(&self) -> Vec<super::message::RichTextSegment> {
        self.get_message().extract_rich_text()
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
/// async fn handler(event: EventContext<PrivateMessage>) -> Outcome {
///     // Access fields directly via Deref
///     println!("From: {} Message: {}", event.user_id, event.get_plain_text());
///     
///     // The event can be passed directly to APIs
///     bot.send(&event, "reply").await.ok();
///     
///     Outcome::Handled
/// }
/// ```
#[derive(Clone)]
pub struct EventContext<T: Event + Clone> {
    /// The extracted event data.
    data: T,
}

impl<T: Event + Clone> EventContext<T> {
    /// Creates a new EventContext with the given data.
    pub fn new(data: T) -> Self {
        Self { data }
    }

    /// Returns a reference to the event as a trait object.
    pub fn as_event(&self) -> &dyn Event {
        &self.data as &dyn Event
    }
}

impl<T: Event + Clone> Deref for EventContext<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Event + Clone + std::fmt::Debug> std::fmt::Debug for EventContext<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventContext")
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Event + Clone> AsRef<dyn Event> for EventContext<T> {
    fn as_ref(&self) -> &dyn Event {
        &self.data
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
/// let text = event.get_plain_text();
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
    pub fn extract<E: FromEvent + Event>(&self) -> Option<EventContext<E>> {
        E::from_event(self.inner.as_ref()).map(EventContext::new)
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
