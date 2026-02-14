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
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_core::{Message, MessageSegment};
//!
//! // Define your segment type
//! #[derive(Debug, Clone)]
//! enum MySegment {
//!     Text(String),
//!     Image(String),
//! }
//!
//! impl MessageSegment for MySegment {
//!     fn segment_type(&self) -> &str {
//!         match self {
//!             MySegment::Text(_) => "text",
//!             MySegment::Image(_) => "image",
//!         }
//!     }
//!     
//!     fn as_text(&self) -> Option<&str> {
//!         match self {
//!             MySegment::Text(s) => Some(s),
//!             _ => None,
//!         }
//!     }
//!     
//!     fn display(&self) -> String {
//!         match self {
//!             MySegment::Text(s) => s.clone(),
//!             MySegment::Image(url) => format!("[Image: {}]", url),
//!         }
//!     }
//! }
//!
//! // Use the generic Message type
//! type MyMessage = Message<MySegment>;
//!
//! let msg = MyMessage::new()
//!     .push(MySegment::Text("Hello".to_string()))
//!     .push(MySegment::Image("http://...".to_string()));
//! ```

use std::fmt::Debug;
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
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::RichTextSegment;
///
/// let segments: Vec<RichTextSegment> = vec![
///     RichTextSegment::Text("Hello ".into()),
///     RichTextSegment::At("12345".into()),
///     RichTextSegment::Text(" check this: ".into()),
///     RichTextSegment::Image("abc.jpg".into()),
/// ];
/// ```
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
pub trait MessageSegment: Debug + Clone + Send + Sync + 'static {
    /// Returns the type identifier of this segment (e.g., "text", "image", "at").
    fn segment_type(&self) -> &str;

    /// Returns true if this is a plain text segment.
    fn is_text(&self) -> bool {
        self.segment_type() == "text"
    }

    /// Returns the text content if this is a text segment.
    fn as_text(&self) -> Option<&str>;

    /// Returns a string representation suitable for display.
    fn display(&self) -> String;

    /// Converts this segment into a platform-agnostic [`RichTextSegment`].
    ///
    /// The default implementation returns `RichTextSegment::Text` using
    /// [`as_text()`](MessageSegment::as_text) (or an empty string for non-text segments).
    /// Adapters should override this to properly convert image and at-mention
    /// segments into their rich-text equivalents.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// impl MessageSegment for MySegment {
    ///     fn as_rich_text(&self) -> RichTextSegment {
    ///         match self {
    ///             MySegment::Text(s) => RichTextSegment::Text(s.clone()),
    ///             MySegment::Image(data) => RichTextSegment::Image(data.file.clone()),
    ///             MySegment::At(data) => RichTextSegment::At(data.id.clone()),
    ///             _ => RichTextSegment::Text(self.display()),
    ///         }
    ///     }
    /// }
    /// ```
    fn as_rich_text(&self) -> RichTextSegment {
        RichTextSegment::Text(self.as_text().unwrap_or_default().to_string())
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
        self.iter().map(MessageSegment::as_rich_text).collect()
    }

    /// Returns a display string representation of the entire message.
    pub fn display(&self) -> String {
        self.iter().map(MessageSegment::display).collect()
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
