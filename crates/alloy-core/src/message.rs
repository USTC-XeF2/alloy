//! Message types for the Alloy framework.
//!
//! This module provides the core message abstraction that enables
//! cross-protocol message handling.
//!
//! # Architecture
//!
//! The message system is built around two core abstractions:
//! - [`ReceiveMessageSegment`] / [`SendMessageSegment`]: Traits for incoming/outgoing
//!   message segments
//! - [`Message<S>`]: A generic struct holding a collection of segments
//!
//! Protocol adapters define their own segment types and use `Message<TheirSegment>`.

use std::borrow::Cow;
use std::fmt::Display;

use derive_more::{AsMut, AsRef, Deref, DerefMut, From, Into, IntoIterator};
use downcast_rs::{Downcast, impl_downcast};
use serde::{Deserialize, Serialize};

// ============================================================================
// Rich Text Segment
// ============================================================================

/// A platform-agnostic rich text segment.
///
/// This enum provides a unified representation of message segments across
/// all adapters. Adapters can convert their platform-specific segments
/// into `RichTextSegment` via [`ReceiveMessageSegment::as_rich_text()`].
///
/// # Variants
///
/// - `Text`: Plain text content
/// - `Image`: An image, identified by a platform-specific reference string
/// - `At`: A user mention, identified by a user ID string
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichTextSegment {
    /// Plain text content.
    Text(String),
    /// An image segment. The string is a platform-specific reference
    /// (file path, URL, base64, etc.).
    Image(String),
    /// A user mention. The string is the user identifier
    /// (e.g., QQ number, Discord user ID).
    At(String),
    /// A segment representing an @all mention.
    AtAll,
    /// A segment representing a quote reply. The string is the quoted message ID.
    Reply(String),
}

// ============================================================================
// Message Segment Traits
// ============================================================================

/// A trait for segments received from an adapter/protocol.
pub trait ReceiveMessageSegment: Send + Sync + 'static {
    /// Returns the type identifier of this segment (e.g., "text", "image", "at").
    fn segment_type(&self) -> &str;

    /// Returns true if this is a plain text segment.
    fn is_text(&self) -> bool {
        self.segment_type() == "text"
    }

    /// Returns the text content if this is a text segment.
    fn as_text(&self) -> Option<&str>;

    /// Converts this segment into a platform-agnostic [`RichTextSegment`].
    fn as_rich_text(&self) -> Option<RichTextSegment>;
}

/// A trait for segments that can be constructed and sent via an adapter.
pub trait SendMessageSegment: ReceiveMessageSegment + Clone {
    fn text(text: impl Into<String>) -> Self;

    /// Attempts to construct a segment from a platform-agnostic [`RichTextSegment`].
    ///
    /// `Text` segments should always be convertible. `Image` and `At` segments
    /// should be converted where the protocol supports them.
    fn from_rich_text_segment(seg: &RichTextSegment) -> Option<Self>;
}

// ============================================================================
// RichTextSegment as first-class receive/send segment
// ============================================================================

impl ReceiveMessageSegment for RichTextSegment {
    fn segment_type(&self) -> &str {
        match self {
            RichTextSegment::Text(_) => "text",
            RichTextSegment::Image(_) => "image",
            RichTextSegment::At(_) => "at",
            RichTextSegment::AtAll => "at_all",
            RichTextSegment::Reply(_) => "reply",
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            RichTextSegment::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Identity conversion — a `RichTextSegment` is already its own rich form.
    fn as_rich_text(&self) -> Option<RichTextSegment> {
        Some(self.clone())
    }
}

impl SendMessageSegment for RichTextSegment {
    fn text(text: impl Into<String>) -> Self {
        RichTextSegment::Text(text.into())
    }

    /// Identity: `RichTextSegment` can always be constructed from itself.
    fn from_rich_text_segment(seg: &RichTextSegment) -> Option<Self> {
        Some(seg.clone())
    }
}

impl Display for RichTextSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RichTextSegment::Text(s) => write!(f, "{s}"),
            RichTextSegment::Image(r) => write!(f, "[Image: {r}]"),
            RichTextSegment::At(id) => write!(f, "@{id}"),
            RichTextSegment::AtAll => write!(f, "@all"),
            RichTextSegment::Reply(id) => write!(f, "[Reply to {id}]"),
        }
    }
}

// ============================================================================
// Message Generic Struct
// ============================================================================

/// A generic message type composed of segments.
///
/// This struct provides common message functionality for all adapters.
/// Each adapter uses `Message<TheirSegmentType>` and can implement
/// adapter-specific methods via `impl Message<TheirSegment>`.
#[derive(
    Debug, Clone, Serialize, Deserialize, Deref, DerefMut, AsRef, AsMut, From, Into, IntoIterator,
)]
#[serde(transparent)]
pub struct Message<S> {
    segments: Vec<S>,
}

impl<S: ReceiveMessageSegment> Message<S> {
    /// Extracts all plain text content from the message.
    ///
    /// This concatenates the text content of all text segments,
    /// ignoring non-text segments like images or mentions.
    pub fn extract_plain_text(&self) -> String {
        self.iter()
            .filter_map(ReceiveMessageSegment::as_text)
            .collect()
    }

    /// Extracts rich text segments from the message.
    ///
    /// Converts each platform-specific segment into a [`RichTextSegment`]
    /// using [`ReceiveMessageSegment::as_rich_text()`].
    pub fn extract_rich_text(&self) -> RichText {
        self.iter()
            .filter_map(ReceiveMessageSegment::as_rich_text)
            .collect()
    }
}

impl<S: SendMessageSegment> Message<S> {
    /// Creates a message from a type-erased `ErasedMessage`.
    ///
    /// This attempts to downcast the `ErasedMessage` to `Message<S>`. If the downcast
    /// fails, it tries to convert from rich text segments using `S::from_rich_text_segment`.
    pub fn from_erased_message(msg: &dyn Sendable) -> Cow<'_, Self> {
        if let Some(msg) = msg.downcast_ref::<Self>() {
            Cow::Borrowed(msg)
        } else {
            Cow::Owned(
                msg.extract_rich_text()
                    .iter()
                    .filter_map(S::from_rich_text_segment)
                    .collect(),
            )
        }
    }

    /// Adds a segment to the end of the message.
    pub fn push(&mut self, segment: S) {
        self.segments.push(segment);
    }

    /// Consumes the message and adds a segment (builder pattern).
    pub fn with(mut self, segment: S) -> Self {
        self.segments.push(segment);
        self
    }
}

impl<S> Message<S> {
    /// Creates a new empty message.
    pub const fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }
}

impl<S> Default for Message<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Display> Display for Message<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.segments.iter().try_for_each(|seg| write!(f, "{seg}"))
    }
}

impl<S> From<S> for Message<S> {
    fn from(segment: S) -> Self {
        Self {
            segments: vec![segment],
        }
    }
}

impl<S> FromIterator<S> for Message<S> {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self {
            segments: iter.into_iter().collect(),
        }
    }
}

// ============================================================================
// RichText — non-generic, protocol-agnostic message type
// ============================================================================

/// A protocol-agnostic message composed of [`RichTextSegment`]s.
///
/// Handlers can return `RichText` (or `Result<RichText, E>`) and the
/// framework will deliver it via [`Bot::send_message`]. Each adapter
/// converts it to its native format via
/// [`SendMessageSegment::from_rich_text_segment`]; unknown segment kinds are
/// silently dropped and the adapter falls back to plain text if the result
/// would be empty.
pub type RichText = Message<RichTextSegment>;

impl RichText {
    pub fn text(self, text: impl Into<String>) -> Self {
        self.with(RichTextSegment::Text(text.into()))
    }

    pub fn image(self, reference: impl Into<String>) -> Self {
        self.with(RichTextSegment::Image(reference.into()))
    }

    pub fn at(self, id: impl Into<String>) -> Self {
        self.with(RichTextSegment::At(id.into()))
    }

    pub fn at_all(self) -> Self {
        self.with(RichTextSegment::AtAll)
    }

    pub fn reply(self, message_id: impl Into<String>) -> Self {
        self.with(RichTextSegment::Reply(message_id.into()))
    }
}

// ============================================================================
// ErasedMessage — type-erased message for object-safe Bot::send_message
// ============================================================================

/// Object-safe, type-erased message trait.
///
/// This trait allows [`Bot::send`] to accept any `Message<S>` without making
/// the trait generic (which would break object safety).
///
/// Concrete adapter implementations can downcast using `downcast_rs` methods
/// (e.g., `downcast_ref::<T>()`) to recover the original typed message.
/// If the downcast fails they should fall back to [`ErasedMessage::extract_rich_text`].
pub trait Sendable: Downcast + Send + Sync {
    /// Extracts platform-agnostic rich text segments from the message.
    fn extract_rich_text(&self) -> RichText;
}

impl_downcast!(Sendable);

impl Sendable for String {
    fn extract_rich_text(&self) -> RichText {
        RichTextSegment::text(self).into()
    }
}

impl<S: ReceiveMessageSegment> Sendable for Message<S> {
    fn extract_rich_text(&self) -> RichText {
        Message::extract_rich_text(self)
    }
}
