//! Message types for the Alloy framework.
//!
//! This module provides the core message abstraction that enables
//! cross-protocol message handling.
//!
//! # Architecture
//!
//! The message system is built around two core abstractions:
//! - [`MessageSegment`]: A trait for a single unit of content (text, image, etc.)
//! - [`Message<S>`]: A generic struct holding a collection of segments
//!
//! Protocol adapters define their own segment types and use `Message<TheirSegment>`.

use std::any::Any;
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

// ============================================================================
// Rich Text Segment
// ============================================================================

/// A platform-agnostic rich text segment.
///
/// This enum provides a unified representation of message segments across
/// all adapters. Adapters can convert their platform-specific segments
/// into `RichTextSegment` via [`MessageSegment::as_rich_text()`].
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
}

// ============================================================================
// Message Segment Trait
// ============================================================================

/// A trait representing a single segment of a message.
///
/// A message segment is the smallest unit of content in a message.
/// It can be plain text, an image, an emoji, a mention, etc.
///
/// Protocol adapters should implement this trait for their segment types.
pub trait MessageSegment: Debug + Clone + Display + Send + Sync + 'static {
    fn text(text: impl Into<String>) -> Self;

    /// Returns the type identifier of this segment (e.g., "text", "image", "at").
    fn segment_type(&self) -> &str;

    /// Returns true if this is a plain text segment.
    fn is_text(&self) -> bool {
        self.segment_type() == "text"
    }

    /// Returns the text content if this is a text segment.
    fn as_text(&self) -> Option<&str>;

    /// Converts this segment into a platform-agnostic [`RichTextSegment`].
    ///
    /// The default implementation returns `Some(RichTextSegment::Text)` if this is a
    /// text segment (via [`as_text()`](MessageSegment::as_text)), or `None` otherwise.
    /// Adapters should override this to properly convert image and at-mention
    /// segments into their rich-text equivalents.
    fn as_rich_text(&self) -> Option<RichTextSegment> {
        self.as_text().map(RichTextSegment::text)
    }

    /// Attempts to construct a segment from a platform-agnostic [`RichTextSegment`].
    ///
    /// The default implementation returns `None` (no conversion possible).
    /// Adapters should override this to support cross-protocol message forwarding.
    ///
    /// `Text` segments should always be convertible. `Image` and `At` segments
    /// should be converted where the protocol supports them.
    fn from_rich_text_segment(seg: &RichTextSegment) -> Option<Self> {
        if let RichTextSegment::Text(s) = seg {
            Some(Self::text(s))
        } else {
            None
        }
    }
}

// ============================================================================
// RichTextSegment as a first-class MessageSegment
// ============================================================================

impl MessageSegment for RichTextSegment {
    fn text(text: impl Into<String>) -> Self {
        RichTextSegment::Text(text.into())
    }

    fn segment_type(&self) -> &str {
        match self {
            RichTextSegment::Text(_) => "text",
            RichTextSegment::Image(_) => "image",
            RichTextSegment::At(_) => "at",
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
///
/// # Type Parameters
///
/// - `S`: The segment type, must implement [`MessageSegment`]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Message<S: MessageSegment> {
    #[serde(bound(deserialize = "S: Deserialize<'de>"))]
    segments: Vec<S>,
}

impl<S: MessageSegment> Message<S> {
    /// Creates a new empty message.
    pub const fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Creates a message from a vector of segments.
    pub fn from_segments(segments: Vec<S>) -> Self {
        Self { segments }
    }

    /// Creates a message from a type-erased `ErasedMessage`.
    ///
    /// This attempts to downcast the `ErasedMessage` to `Message<S>`. If the downcast
    /// fails, it tries to convert from rich text segments using `S::from_rich_text_segment`.
    pub fn from_erased_message(msg: &dyn ErasedMessage) -> Self {
        if let Some(msg) = msg.as_any().downcast_ref::<Self>() {
            msg.clone()
        } else {
            Self::from_segments(
                msg.extract_rich_text()
                    .iter()
                    .filter_map(S::from_rich_text_segment)
                    .collect(),
            )
        }
    }

    /// Extracts all plain text content from the message.
    ///
    /// This concatenates the text content of all text segments,
    /// ignoring non-text segments like images or mentions.
    pub fn extract_plain_text(&self) -> String {
        self.iter().filter_map(|seg| seg.as_text()).collect()
    }

    /// Extracts rich text segments from the message.
    ///
    /// Converts each platform-specific segment into a [`RichTextSegment`]
    /// using [`MessageSegment::as_rich_text()`].
    pub fn extract_rich_text(&self) -> Vec<RichTextSegment> {
        self.iter()
            .filter_map(MessageSegment::as_rich_text)
            .collect()
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

    /// Consumes the message and returns the inner segments vector.
    pub fn into_segments(self) -> Vec<S> {
        self.segments
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Deref implementations
// ══════════════════════════════════════════════════════════════════════════════

impl<S: MessageSegment> Deref for Message<S> {
    type Target = [S];

    fn deref(&self) -> &Self::Target {
        &self.segments
    }
}

impl<S: MessageSegment> DerefMut for Message<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.segments
    }
}

impl<S: MessageSegment> Display for Message<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for segment in &self.segments {
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

impl<S: MessageSegment> From<Vec<S>> for Message<S> {
    fn from(segments: Vec<S>) -> Self {
        Self { segments }
    }
}

impl<S: MessageSegment> From<S> for Message<S> {
    fn from(segment: S) -> Self {
        Self {
            segments: vec![segment],
        }
    }
}

impl<S: MessageSegment> FromIterator<S> for Message<S> {
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
/// [`MessageSegment::from_rich_text_segment`]; unknown segment kinds are
/// silently dropped and the adapter falls back to plain text if the result
/// would be empty.
pub type RichText = Message<RichTextSegment>;

impl RichText {
    /// Adds a text segment.
    pub fn text(self, text: impl Into<String>) -> Self {
        self.with(RichTextSegment::Text(text.into()))
    }

    /// Adds an at-mention segment.
    pub fn at(self, id: impl Into<String>) -> Self {
        self.with(RichTextSegment::At(id.into()))
    }

    /// Adds an image segment.
    pub fn image(self, reference: impl Into<String>) -> Self {
        self.with(RichTextSegment::Image(reference.into()))
    }

    /// A convenience constructor for a simple message with optional at-mention.
    pub fn msg(text: impl Into<String>, at: Option<impl Into<String>>) -> Self {
        let mut msg = Self::new();
        if let Some(id) = at {
            msg = msg.at(id);
        }
        msg.text(text)
    }
}

// ============================================================================
// ErasedMessage — type-erased message for object-safe Bot::send_message
// ============================================================================

/// Object-safe, type-erased message trait.
///
/// This trait allows [`Bot::send_message`] to accept any `Message<S>` without
/// making the trait generic (which would break object safety).
///
/// Concrete adapter implementations can downcast via [`ErasedMessage::as_any`]
/// to recover the original typed message. If the downcast fails they should
/// fall back to [`ErasedMessage::extract_rich_text`].
pub trait ErasedMessage: Any + Send + Sync {
    /// Returns a `&dyn Any` reference for downcasting to the concrete message type.
    fn as_any(&self) -> &dyn Any;

    /// Extracts platform-agnostic rich text segments from the message.
    fn extract_rich_text(&self) -> Vec<RichTextSegment>;
}

impl<S: MessageSegment> ErasedMessage for Message<S> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn extract_rich_text(&self) -> Vec<RichTextSegment> {
        Message::extract_rich_text(self)
    }
}
