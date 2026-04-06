//! OneBot v11 Message type.
//!
//! This module provides OneBot-specific extensions for `Message<Segment>`.
//!
//! # Message Formats
//!
//! OneBot v11 supports two message formats:
//! - **Array format**: A JSON array of message segments (recommended)
//! - **String format**: CQ-coded string (legacy, requires `cqcode` feature)
//!
//! When `cqcode` is enabled, this module supports both formats via custom serde helpers.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::{OneBotMessage, Segment, OneBotMessageExt};
//!
//! // Create a message
//! let msg = OneBotMessage::from_segments(vec![
//!     Segment::text("Hello, "),
//!     Segment::at(10001000),
//! ]);
//!
//! // Use extension methods
//! #[cfg(feature = "cqcode")]
//! println!("CQ string: {}", msg.to_cq_string());
//! println!("Mentioned users: {:?}", msg.mentioned_users());
//! ```

use alloy_core::Message;

use super::segment::Segment;

// ============================================================================
// Type Alias
// ============================================================================

/// A OneBot v11 message composed of multiple segments.
///
/// This is a type alias for `Message<Segment>`. Use the `OneBotMessageExt`
/// trait to access OneBot-specific methods.
pub type OneBotMessage = Message<Segment>;

// ============================================================================
// Extension Trait (avoids orphan rule for OneBot-specific methods)
// ============================================================================

/// Extension trait providing OneBot-specific methods for `Message<Segment>`.
pub trait OneBotMessageExt {
    /// Returns all @mention QQ numbers in the message.
    fn mentioned_users(&self) -> Vec<i64>;

    /// Checks if the message contains @all.
    fn mentions_all(&self) -> bool;

    /// Gets the reply message ID if this is a reply.
    fn reply_to(&self) -> Option<&str>;

    /// Converts the message to CQ code string format.
    #[cfg(feature = "cqcode")]
    fn to_cq_string(&self) -> String;
}

impl OneBotMessageExt for OneBotMessage {
    fn mentioned_users(&self) -> Vec<i64> {
        self.iter()
            .filter_map(|seg| {
                if let Segment::At(data) = seg {
                    if data.qq == "all" {
                        None
                    } else {
                        data.qq.parse().ok()
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    fn mentions_all(&self) -> bool {
        self.iter()
            .any(|seg| matches!(seg, Segment::At(data) if data.qq == "all"))
    }

    fn reply_to(&self) -> Option<&str> {
        self.iter().find_map(|seg| {
            if let Segment::Reply(data) = seg {
                Some(data.id.as_str())
            } else {
                None
            }
        })
    }

    #[cfg(feature = "cqcode")]
    fn to_cq_string(&self) -> String {
        self.iter().map(Segment::to_cq_code).collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use alloy_core::MessageSegment;

    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = OneBotMessage::from_segments(vec![
            Segment::text("Hello, "),
            Segment::at(10001000),
            Segment::text("!"),
        ]);

        assert_eq!(msg.len(), 3);
        assert_eq!(msg.extract_plain_text(), "Hello, !");
    }

    #[test]
    fn test_message_serialize_array() {
        let msg = OneBotMessage::from_segments(vec![Segment::text("Hello"), Segment::face(178)]);
        // Serialize using slice to get array format
        let json = serde_json::to_string(&msg[..]).unwrap();
        assert_eq!(
            json,
            r#"[{"type":"text","data":{"text":"Hello"}},{"type":"face","data":{"id":"178"}}]"#
        );
    }

    #[test]
    fn test_message_deserialize_array() {
        let json =
            r#"[{"type":"text","data":{"text":"Hello"}},{"type":"at","data":{"qq":"10001000"}}]"#;
        let msg: OneBotMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.len(), 2);
        assert_eq!(msg.extract_plain_text(), "Hello");
    }

    #[test]
    fn test_mentioned_users() {
        let msg = OneBotMessage::from_segments(vec![
            Segment::at(10001000),
            Segment::text(" and "),
            Segment::at(10001001),
            Segment::at_all(),
        ]);

        let users = msg.mentioned_users();
        assert_eq!(users, vec![10001000, 10001001]);
        assert!(msg.mentions_all());
    }

    #[test]
    fn test_reply_to() {
        let msg = OneBotMessage::from_segments(vec![
            Segment::reply("12345"),
            Segment::text("This is a reply"),
        ]);

        assert_eq!(msg.reply_to(), Some("12345"));
    }

    #[test]
    fn test_message_methods() {
        let msg = OneBotMessage::from_segments(vec![
            Segment::text("Hello"),
            Segment::image("test.jpg"),
            Segment::text(" World"),
        ]);

        // Test Message core methods
        assert_eq!(msg.len(), 3);
        assert!(!msg.is_empty());
        assert_eq!(msg.extract_plain_text(), "Hello World");
        assert_eq!(msg.len(), 3);
    }

    #[test]
    fn test_extension_trait() {
        let msg = OneBotMessage::from_segments(vec![Segment::text("Plain text")]);

        // Test extension trait methods
        assert_eq!(msg.mentioned_users(), Vec::<i64>::new());
        assert!(!msg.mentions_all());
        assert_eq!(msg.reply_to(), None);
    }
}
