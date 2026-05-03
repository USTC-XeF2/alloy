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
//! use amira_adapter_onebot::{OneBotMessage, Segment, OneBotMessageExt};
//!
//! // Create a message
//! let msg = OneBotMessage::from(vec![
//!     Segment::text("Hello, "),
//!     Segment::at(10001000),
//! ]);
//!
//! // Use extension methods
//! #[cfg(feature = "cqcode")]
//! println!("CQ string: {}", msg.to_cq_string());
//! println!("Mentioned users: {:?}", msg.mentioned_users());
//! ```

use amira_core::Message;

use super::segment::Segment;

/// A OneBot v11 message composed of multiple segments.
///
/// This is a type alias for `Message<Segment>`. Use the `OneBotMessageExt`
/// trait to access OneBot-specific methods.
pub type OneBotMessage = Message<Segment>;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentioned_users() {
        let msg = OneBotMessage::from(vec![
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
        let msg = OneBotMessage::from(vec![
            Segment::reply("12345"),
            Segment::text("This is a reply"),
        ]);

        assert_eq!(msg.reply_to(), Some("12345"));
    }
}
