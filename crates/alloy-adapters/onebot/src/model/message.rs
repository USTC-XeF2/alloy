//! OneBot v11 Message type.
//!
//! This module provides OneBot-specific extensions for `Message<Segment>`.
//!
//! # Message Formats
//!
//! OneBot v11 supports two message formats:
//! - **Array format**: A JSON array of message segments (recommended)
//! - **String format**: CQ-coded string (legacy, for compatibility)
//!
//! This module handles both formats via custom serde helpers.
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
//! println!("CQ string: {}", msg.to_cq_string());
//! println!("Mentioned users: {:?}", msg.mentioned_users());
//! ```

use alloy_core::Message;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::segment::{Segment, unescape_cq_text, unescape_cq_value};

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
    /// Converts the message to CQ code string format.
    fn to_cq_string(&self) -> String;

    /// Returns all @mention QQ numbers in the message.
    fn mentioned_users(&self) -> Vec<i64>;

    /// Checks if the message contains @all.
    fn mentions_all(&self) -> bool;

    /// Gets the reply message ID if this is a reply.
    fn reply_to(&self) -> Option<&str>;
}

impl OneBotMessageExt for Message<Segment> {
    fn to_cq_string(&self) -> String {
        self.iter().map(Segment::to_cq_code).collect()
    }

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
}

// ============================================================================
// Serde helpers (for use with #[serde(with = "...")])
// ============================================================================

/// Serde helper module for OneBot message serialization.
///
/// Use with `#[serde(with = "crate::model::message::serde_message")]` on message fields.
pub mod serde_message {
    use super::{Deserialize, Deserializer, Message, Segment, Serialize, Serializer};

    pub fn serialize<S>(msg: &Message<Segment>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Always serialize as array format (serialize the slice, not the struct)
        msg[..].serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Message<Segment>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Support both array and string formats
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum MessageFormat {
            Array(Vec<Segment>),
            String(String),
        }

        match MessageFormat::deserialize(deserializer)? {
            MessageFormat::Array(segments) => Ok(Message::from_segments(segments)),
            MessageFormat::String(cq_string) => {
                let segments = super::parse_cq_string(&cq_string);
                Ok(Message::from_segments(segments))
            }
        }
    }
}

// ============================================================================
// CQ Code Parsing
// ============================================================================

/// Parses a CQ code string into a vector of segments.
///
/// This handles the string format where text and CQ codes are mixed:
/// ```text
/// Hello [CQ:face,id=178] World [CQ:at,qq=10001000]
/// ```
pub fn parse_cq_string(input: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    while pos < len {
        // Look for CQ code start
        if pos + 4 <= len && chars[pos..pos + 4] == ['[', 'C', 'Q', ':'] {
            // Find the matching ]
            let start = pos;
            pos += 4; // Skip [CQ:

            // Find the function name (up to , or ])
            let func_start = pos;
            while pos < len && chars[pos] != ',' && chars[pos] != ']' {
                pos += 1;
            }
            let func_name: String = chars[func_start..pos].iter().collect();

            // Parse parameters
            let mut params: Vec<(String, String)> = Vec::new();
            while pos < len && chars[pos] == ',' {
                pos += 1; // Skip ,

                // Find parameter name (up to =)
                let param_start = pos;
                while pos < len && chars[pos] != '=' && chars[pos] != ']' {
                    pos += 1;
                }
                let param_name: String = chars[param_start..pos].iter().collect();

                if pos < len && chars[pos] == '=' {
                    pos += 1; // Skip =

                    // Find parameter value (up to , or ])
                    let value_start = pos;
                    while pos < len && chars[pos] != ',' && chars[pos] != ']' {
                        pos += 1;
                    }
                    let param_value: String = chars[value_start..pos].iter().collect();
                    let param_value = unescape_cq_value(&param_value);
                    params.push((param_name, param_value));
                }
            }

            // Skip the closing ]
            if pos < len && chars[pos] == ']' {
                pos += 1;
            }

            // Create segment from parsed CQ code
            if let Some(segment) = cq_to_segment(&func_name, &params) {
                segments.push(segment);
            } else {
                // Unknown CQ code, treat as text
                let text: String = chars[start..pos].iter().collect();
                segments.push(Segment::text(text));
            }
        } else {
            // Regular text - collect until [ or end
            let start = pos;
            while pos < len && !(pos + 4 <= len && chars[pos..pos + 4] == ['[', 'C', 'Q', ':']) {
                pos += 1;
            }
            let text: String = chars[start..pos].iter().collect();
            let text = unescape_cq_text(&text);
            if !text.is_empty() {
                segments.push(Segment::text(text));
            }
        }
    }

    segments
}

/// Converts a parsed CQ code into a segment.
fn cq_to_segment(func: &str, params: &[(String, String)]) -> Option<Segment> {
    let get = |key: &str| -> Option<&str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };

    match func {
        "face" => Some(Segment::Face(super::segment::FaceData {
            id: get("id")?.to_string(),
        })),
        "image" => Some(Segment::Image(super::segment::ImageData {
            file: get("file")?.to_string(),
            image_type: get("type").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "record" => Some(Segment::Record(super::segment::RecordData {
            file: get("file")?.to_string(),
            magic: get("magic").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "video" => Some(Segment::Video(super::segment::VideoData {
            file: get("file")?.to_string(),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "at" => Some(Segment::At(super::segment::AtData {
            qq: get("qq")?.to_string(),
        })),
        "rps" => Some(Segment::Rps(super::segment::RpsData {})),
        "dice" => Some(Segment::Dice(super::segment::DiceData {})),
        "shake" => Some(Segment::Shake(super::segment::ShakeData {})),
        "poke" => Some(Segment::Poke(super::segment::PokeData {
            poke_type: get("type")?.to_string(),
            id: get("id")?.to_string(),
            name: get("name").map(ToString::to_string),
        })),
        "anonymous" => Some(Segment::Anonymous(super::segment::AnonymousData {
            ignore: get("ignore").map(ToString::to_string),
        })),
        "share" => Some(Segment::Share(super::segment::ShareData {
            url: get("url")?.to_string(),
            title: get("title")?.to_string(),
            content: get("content").map(ToString::to_string),
            image: get("image").map(ToString::to_string),
        })),
        "contact" => Some(Segment::Contact(super::segment::ContactData {
            contact_type: get("type")?.to_string(),
            id: get("id")?.to_string(),
        })),
        "location" => Some(Segment::Location(super::segment::LocationData {
            lat: get("lat")?.to_string(),
            lon: get("lon")?.to_string(),
            title: get("title").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
        })),
        "music" => Some(Segment::Music(super::segment::MusicData {
            music_type: get("type")?.to_string(),
            id: get("id").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            audio: get("audio").map(ToString::to_string),
            title: get("title").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
            image: get("image").map(ToString::to_string),
        })),
        "reply" => Some(Segment::Reply(super::segment::ReplyData {
            id: get("id")?.to_string(),
        })),
        "forward" => Some(Segment::Forward(super::segment::ForwardData {
            id: get("id")?.to_string(),
        })),
        "node" => Some(Segment::Node(super::segment::NodeData {
            id: get("id").map(ToString::to_string),
            user_id: get("user_id").map(ToString::to_string),
            nickname: get("nickname").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
        })),
        "xml" => Some(Segment::Xml(super::segment::XmlData {
            data: get("data")?.to_string(),
        })),
        "json" => Some(Segment::Json(super::segment::JsonData {
            data: get("data")?.to_string(),
        })),
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::segment::TextData;
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
        let msg =
            serde_message::deserialize(&mut serde_json::Deserializer::from_str(json)).unwrap();
        assert_eq!(msg.len(), 2);
        assert_eq!(msg.extract_plain_text(), "Hello");
    }

    #[test]
    fn test_message_deserialize_string() {
        let json = r#""Hello [CQ:face,id=178] World""#;
        let msg =
            serde_message::deserialize(&mut serde_json::Deserializer::from_str(json)).unwrap();
        assert_eq!(msg.len(), 3);
        assert_eq!(msg.extract_plain_text(), "Hello  World");
    }

    #[test]
    fn test_parse_cq_string() {
        let segments = parse_cq_string("Hello [CQ:face,id=178] World");
        assert_eq!(segments.len(), 3);
        assert!(matches!(&segments[0], Segment::Text(TextData { text }) if text == "Hello "));
        assert!(matches!(&segments[1], Segment::Face(_)));
        assert!(matches!(&segments[2], Segment::Text(TextData { text }) if text == " World"));
    }

    #[test]
    fn test_parse_cq_string_complex() {
        let segments = parse_cq_string("[CQ:at,qq=10001000]你好[CQ:image,file=123.jpg]");
        assert_eq!(segments.len(), 3);
        assert!(matches!(&segments[0], Segment::At(_)));
        assert!(matches!(&segments[1], Segment::Text(_)));
        assert!(matches!(&segments[2], Segment::Image(_)));
    }

    #[test]
    fn test_to_cq_string() {
        let msg = OneBotMessage::from_segments(vec![
            Segment::text("Hello "),
            Segment::face(178),
            Segment::text(" World"),
        ]);
        assert_eq!(msg.to_cq_string(), "Hello [CQ:face,id=178] World");
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

    #[test]
    fn test_cq_escaping() {
        let segments = parse_cq_string("&#91;escaped&#93; &amp; test");
        assert_eq!(segments.len(), 1);
        assert!(
            matches!(&segments[0], Segment::Text(TextData { text }) if text == "[escaped] & test")
        );
    }
}
