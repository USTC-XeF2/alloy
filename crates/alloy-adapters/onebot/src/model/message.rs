//! OneBot v11 Message type.
//!
//! This module provides the [`OneBotMessage`] type that implements
//! the [`alloy_core::Message`] trait, representing a complete message
//! composed of multiple segments.
//!
//! # Message Formats
//!
//! OneBot v11 supports two message formats:
//! - **Array format**: A JSON array of message segments (recommended)
//! - **String format**: CQ-coded string (legacy, for compatibility)
//!
//! This module handles both formats transparently.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::{OneBotMessage, Segment};
//! use alloy_core::Message;
//!
//! // Create a message with builder pattern
//! let msg = OneBotMessage::new()
//!     .text("Hello, ")
//!     .at(10001000)
//!     .text("! Check this out: ")
//!     .image("http://example.com/image.jpg");
//!
//! // Access segments
//! for segment in msg.iter() {
//!     println!("{}", segment.display());
//! }
//!
//! // Extract plain text
//! println!("Plain text: {}", msg.extract_plain_text());
//! ```

use alloy_core::{Message as MessageTrait, MessageSegment as MessageSegmentTrait};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::segment::Segment;

// ============================================================================
// OneBotMessage - The main message type
// ============================================================================

/// A OneBot v11 message composed of multiple segments.
///
/// This type implements [`alloy_core::Message`] and provides a builder
/// pattern for constructing messages, as well as utilities for parsing
/// and converting between different message formats.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OneBotMessage {
    /// The segments that make up this message.
    segments: Vec<Segment>,
}

impl MessageTrait for OneBotMessage {
    type Segment = Segment;

    fn iter(&self) -> impl Iterator<Item = &Self::Segment> {
        self.segments.iter()
    }

    fn len(&self) -> usize {
        self.segments.len()
    }

    fn as_slice(&self) -> &[Self::Segment] {
        &self.segments
    }
}

// ============================================================================
// Serialization / Deserialization
// ============================================================================

impl Serialize for OneBotMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Always serialize as array format
        self.segments.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OneBotMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
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
            MessageFormat::Array(segments) => Ok(OneBotMessage { segments }),
            MessageFormat::String(cq_string) => Ok(OneBotMessage::from_cq_string(&cq_string)),
        }
    }
}

// ============================================================================
// Constructors and Builders
// ============================================================================

impl OneBotMessage {
    /// Creates a new empty message.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Creates a message from a vector of segments.
    pub fn from_segments(segments: Vec<Segment>) -> Self {
        Self { segments }
    }

    /// Creates a message containing only plain text.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            segments: vec![Segment::text(text)],
        }
    }

    /// Creates a message from a CQ code string.
    ///
    /// Parses the string format into an array of segments.
    pub fn from_cq_string(cq_string: &str) -> Self {
        Self {
            segments: parse_cq_string(cq_string),
        }
    }

    // --------------------------------
    // Builder methods
    // --------------------------------

    /// Adds a text segment to the message.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.segments.push(Segment::text(text));
        self
    }

    /// Adds a face/emoji segment.
    pub fn face(mut self, id: i32) -> Self {
        self.segments.push(Segment::face(id));
        self
    }

    /// Adds an image segment.
    pub fn image(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::image(file));
        self
    }

    /// Adds a flash image segment.
    pub fn flash_image(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::flash_image(file));
        self
    }

    /// Adds a voice/record segment.
    pub fn record(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::record(file));
        self
    }

    /// Adds a video segment.
    pub fn video(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::video(file));
        self
    }

    /// Adds an @mention segment.
    pub fn at(mut self, qq: i64) -> Self {
        self.segments.push(Segment::at(qq));
        self
    }

    /// Adds an @all segment.
    pub fn at_all(mut self) -> Self {
        self.segments.push(Segment::at_all());
        self
    }

    /// Adds a rock-paper-scissors segment.
    pub fn rps(mut self) -> Self {
        self.segments.push(Segment::rps());
        self
    }

    /// Adds a dice segment.
    pub fn dice(mut self) -> Self {
        self.segments.push(Segment::dice());
        self
    }

    /// Adds a shake segment.
    pub fn shake(mut self) -> Self {
        self.segments.push(Segment::shake());
        self
    }

    /// Adds a poke segment.
    pub fn poke(mut self, poke_type: impl Into<String>, id: impl Into<String>) -> Self {
        self.segments.push(Segment::poke(poke_type, id));
        self
    }

    /// Adds a link share segment.
    pub fn share(mut self, url: impl Into<String>, title: impl Into<String>) -> Self {
        self.segments.push(Segment::share(url, title));
        self
    }

    /// Adds a location segment.
    pub fn location(mut self, lat: f64, lon: f64) -> Self {
        self.segments.push(Segment::location(lat, lon));
        self
    }

    /// Adds a music share segment.
    pub fn music(mut self, music_type: impl Into<String>, id: impl Into<String>) -> Self {
        self.segments.push(Segment::music(music_type, id));
        self
    }

    /// Adds a custom music share segment.
    pub fn music_custom(
        mut self,
        url: impl Into<String>,
        audio: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        self.segments.push(Segment::music_custom(url, audio, title));
        self
    }

    /// Adds a reply segment.
    pub fn reply(mut self, id: impl Into<String>) -> Self {
        self.segments.push(Segment::reply(id));
        self
    }

    /// Adds an XML message segment.
    pub fn xml(mut self, data: impl Into<String>) -> Self {
        self.segments.push(Segment::xml(data));
        self
    }

    /// Adds a JSON message segment.
    pub fn json(mut self, data: impl Into<String>) -> Self {
        self.segments.push(Segment::json(data));
        self
    }

    /// Adds a raw segment.
    pub fn segment(mut self, segment: Segment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Appends multiple segments.
    pub fn append_segments(mut self, segments: impl IntoIterator<Item = Segment>) -> Self {
        self.segments.extend(segments);
        self
    }

    // --------------------------------
    // Mutable builder methods
    // --------------------------------

    /// Adds a text segment to the message (mutable).
    pub fn push_text(&mut self, text: impl Into<String>) -> &mut Self {
        self.segments.push(Segment::text(text));
        self
    }

    /// Adds an @mention segment (mutable).
    pub fn push_at(&mut self, qq: i64) -> &mut Self {
        self.segments.push(Segment::at(qq));
        self
    }

    /// Adds a segment (mutable).
    pub fn push(&mut self, segment: Segment) -> &mut Self {
        self.segments.push(segment);
        self
    }

    /// Extends with multiple segments (mutable).
    pub fn extend(&mut self, segments: impl IntoIterator<Item = Segment>) -> &mut Self {
        self.segments.extend(segments);
        self
    }
}

// ============================================================================
// Conversion Methods
// ============================================================================

impl OneBotMessage {
    /// Returns the segments as a slice.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Returns the segments as a mutable slice.
    pub fn segments_mut(&mut self) -> &mut Vec<Segment> {
        &mut self.segments
    }

    /// Converts the message into a vector of segments.
    pub fn into_segments(self) -> Vec<Segment> {
        self.segments
    }

    /// Converts the message to CQ code string format.
    pub fn to_cq_string(&self) -> String {
        self.segments.iter().map(Segment::to_cq_code).collect()
    }

    /// Returns the first text segment's content, if any.
    pub fn first_text(&self) -> Option<&str> {
        self.segments
            .iter()
            .find_map(|seg| MessageSegmentTrait::as_text(seg))
    }

    /// Checks if the message contains only text segments.
    pub fn is_plain_text(&self) -> bool {
        self.segments.iter().all(Segment::is_text)
    }

    /// Returns all @mention QQ numbers in the message.
    pub fn mentioned_users(&self) -> Vec<i64> {
        self.segments
            .iter()
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

    /// Checks if the message contains @all.
    pub fn mentions_all(&self) -> bool {
        self.segments
            .iter()
            .any(|seg| matches!(seg, Segment::At(data) if data.qq == "all"))
    }

    /// Gets the reply message ID if this is a reply.
    pub fn reply_to(&self) -> Option<&str> {
        self.segments.iter().find_map(|seg| {
            if let Segment::Reply(data) = seg {
                Some(data.id.as_str())
            } else {
                None
            }
        })
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Vec<Segment>> for OneBotMessage {
    fn from(segments: Vec<Segment>) -> Self {
        Self { segments }
    }
}

impl From<Segment> for OneBotMessage {
    fn from(segment: Segment) -> Self {
        Self {
            segments: vec![segment],
        }
    }
}

impl From<&str> for OneBotMessage {
    fn from(text: &str) -> Self {
        Self::from_text(text)
    }
}

impl From<String> for OneBotMessage {
    fn from(text: String) -> Self {
        Self::from_text(text)
    }
}

impl FromIterator<Segment> for OneBotMessage {
    fn from_iter<T: IntoIterator<Item = Segment>>(iter: T) -> Self {
        Self {
            segments: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for OneBotMessage {
    type Item = Segment;
    type IntoIter = std::vec::IntoIter<Segment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.into_iter()
    }
}

impl<'a> IntoIterator for &'a OneBotMessage {
    type Item = &'a Segment;
    type IntoIter = std::slice::Iter<'a, Segment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.iter()
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

/// Unescapes CQ code text.
fn unescape_cq_text(text: &str) -> String {
    text.replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&#44;", ",")
        .replace("&amp;", "&")
}

/// Unescapes CQ code parameter value.
fn unescape_cq_value(value: &str) -> String {
    unescape_cq_text(value)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::segment::TextData;
    use super::*;

    #[test]
    fn test_message_builder() {
        let msg = OneBotMessage::new().text("Hello, ").at(10001000).text("!");

        assert_eq!(msg.len(), 3);
        assert_eq!(msg.extract_plain_text(), "Hello, !");
    }

    #[test]
    fn test_message_serialize_array() {
        let msg = OneBotMessage::new().text("Hello").face(178);
        let json = serde_json::to_string(&msg).unwrap();
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
    fn test_message_deserialize_string() {
        let json = r#""Hello [CQ:face,id=178] World""#;
        let msg: OneBotMessage = serde_json::from_str(json).unwrap();
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
        let msg = OneBotMessage::new().text("Hello ").face(178).text(" World");
        assert_eq!(msg.to_cq_string(), "Hello [CQ:face,id=178] World");
    }

    #[test]
    fn test_mentioned_users() {
        let msg = OneBotMessage::new()
            .at(10001000)
            .text(" and ")
            .at(10001001)
            .at_all();

        let users = msg.mentioned_users();
        assert_eq!(users, vec![10001000, 10001001]);
        assert!(msg.mentions_all());
    }

    #[test]
    fn test_reply_to() {
        let msg = OneBotMessage::new().reply("12345").text("This is a reply");

        assert_eq!(msg.reply_to(), Some("12345"));
    }

    #[test]
    fn test_message_trait() {
        let msg = OneBotMessage::new()
            .text("Hello")
            .image("test.jpg")
            .text(" World");

        // Test Message trait methods
        assert_eq!(msg.len(), 3);
        assert!(!msg.is_empty());
        assert_eq!(msg.extract_plain_text(), "Hello World");
        assert_eq!(msg.as_slice().len(), 3);
    }

    #[test]
    fn test_from_implementations() {
        // From &str
        let msg: OneBotMessage = "Hello".into();
        assert_eq!(msg.extract_plain_text(), "Hello");

        // From String
        let msg: OneBotMessage = String::from("World").into();
        assert_eq!(msg.extract_plain_text(), "World");

        // From Segment
        let msg: OneBotMessage = Segment::face(178).into();
        assert_eq!(msg.len(), 1);

        // From Vec<Segment>
        let msg: OneBotMessage = vec![Segment::text("A"), Segment::text("B")].into();
        assert_eq!(msg.len(), 2);
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
