//! Message traits for the Alloy framework.
//!
//! This module provides the core message abstraction traits that enable
//! cross-protocol message handling.
//!
//! # Architecture
//!
//! The message system is built around two core abstractions:
//! - [`MessageSegment`]: A single unit of content (text, image, etc.)
//! - [`Message`]: A collection of message segments forming a complete message
//!
//! Protocol adapters implement these traits to provide uniform message handling
//! across different chat protocols.

use std::fmt::Debug;

// ============================================================================
// Message Segment Trait
// ============================================================================

/// A trait representing a single segment of a message.
///
/// A message segment is the smallest unit of content in a message.
/// It can be plain text, an image, an emoji, a mention, etc.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::MessageSegment;
///
/// fn process_segment<S: MessageSegment>(segment: &S) {
///     println!("Segment type: {}", segment.segment_type());
///     if segment.is_text() {
///         println!("Text content: {:?}", segment.as_text());
///     }
/// }
/// ```
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
}

// ============================================================================
// Message Trait
// ============================================================================

/// A trait representing a complete message composed of segments.
///
/// A message is a sequence of [`MessageSegment`]s that together form
/// the complete content of a chat message.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::Message;
///
/// fn process_message<M: Message>(msg: &M) {
///     println!("Message has {} segments", msg.len());
///     println!("Plain text: {}", msg.extract_plain_text());
///     
///     for segment in msg.iter() {
///         println!("- {}: {}", segment.segment_type(), segment.display());
///     }
/// }
/// ```
pub trait Message: Debug + Clone + Send + Sync + 'static {
    /// The segment type used by this message.
    type Segment: MessageSegment;

    /// Returns an iterator over the message segments.
    fn iter(&self) -> impl Iterator<Item = &Self::Segment>;

    /// Returns the number of segments in the message.
    fn len(&self) -> usize;

    /// Returns true if the message has no segments.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Extracts all plain text content from the message.
    ///
    /// This concatenates the text content of all text segments,
    /// ignoring non-text segments like images or mentions.
    fn extract_plain_text(&self) -> String {
        self.iter()
            .filter_map(|seg| seg.as_text())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Returns a display string representation of the entire message.
    fn display(&self) -> String {
        self.iter().map(MessageSegment::display).collect()
    }

    /// Returns the segments as a slice (if the underlying storage supports it).
    fn as_slice(&self) -> &[Self::Segment];
}
