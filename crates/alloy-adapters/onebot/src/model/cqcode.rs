use std::fmt::Write;

use alloy_core::{Message, MessageSegment};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::segment::{
    AtData, ContactData, DiceData, FaceData, ForwardData, ImageData, JsonData, LocationData,
    MusicData, NodeData, PokeData, RecordData, ReplyData, RpsData, Segment, ShakeData, ShareData,
    VideoData, XmlData,
};

/// Serde helper module for OneBot message serialization with CQ string compatibility.
///
/// Use with `#[serde(with = "crate::cqcode::serde_message")]` on message fields.
pub(crate) mod serde_message {
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
        "face" => Some(Segment::Face(FaceData {
            id: get("id")?.to_string(),
        })),
        "image" => Some(Segment::Image(ImageData {
            file: get("file")?.to_string(),
            image_type: get("type").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "record" => Some(Segment::Record(RecordData {
            file: get("file")?.to_string(),
            magic: get("magic").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "video" => Some(Segment::Video(VideoData {
            file: get("file")?.to_string(),
            url: get("url").map(ToString::to_string),
            cache: get("cache").map(ToString::to_string),
            proxy: get("proxy").map(ToString::to_string),
            timeout: get("timeout").map(ToString::to_string),
        })),
        "at" => Some(Segment::At(AtData {
            qq: get("qq")?.to_string(),
        })),
        "rps" => Some(Segment::Rps(RpsData {})),
        "dice" => Some(Segment::Dice(DiceData {})),
        "shake" => Some(Segment::Shake(ShakeData {})),
        "poke" => Some(Segment::Poke(PokeData {
            poke_type: get("type")?.to_string(),
            id: get("id")?.to_string(),
            name: get("name").map(ToString::to_string),
        })),
        "share" => Some(Segment::Share(ShareData {
            url: get("url")?.to_string(),
            title: get("title")?.to_string(),
            content: get("content").map(ToString::to_string),
            image: get("image").map(ToString::to_string),
        })),
        "contact" => Some(Segment::Contact(ContactData {
            contact_type: get("type")?.to_string(),
            id: get("id")?.to_string(),
        })),
        "location" => Some(Segment::Location(LocationData {
            lat: get("lat")?.to_string(),
            lon: get("lon")?.to_string(),
            title: get("title").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
        })),
        "music" => Some(Segment::Music(MusicData {
            music_type: get("type")?.to_string(),
            id: get("id").map(ToString::to_string),
            url: get("url").map(ToString::to_string),
            audio: get("audio").map(ToString::to_string),
            title: get("title").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
            image: get("image").map(ToString::to_string),
        })),
        "reply" => Some(Segment::Reply(ReplyData {
            id: get("id")?.to_string(),
        })),
        "forward" => Some(Segment::Forward(ForwardData {
            id: get("id")?.to_string(),
        })),
        "node" => Some(Segment::Node(NodeData {
            id: get("id").map(ToString::to_string),
            user_id: get("user_id").map(ToString::to_string),
            nickname: get("nickname").map(ToString::to_string),
            content: get("content").map(ToString::to_string),
        })),
        "xml" => Some(Segment::Xml(XmlData {
            data: get("data")?.to_string(),
        })),
        "json" => Some(Segment::Json(JsonData {
            data: get("data")?.to_string(),
        })),
        _ => None,
    }
}

impl Segment {
    /// Converts this segment to a CQ code string.
    ///
    /// Text segments are returned as plain text (with escaping).
    /// Other segments are formatted as `[CQ:type,key=value,...]`.
    pub fn to_cq_code(&self) -> String {
        match self {
            Segment::Text(data) => escape_cq_text(&data.text),
            Segment::Face(data) => format!("[CQ:face,id={}]", data.id),
            Segment::Image(data) => {
                let mut cq = format!("[CQ:image,file={}", escape_cq_value(&data.file));
                if let Some(ref t) = data.image_type {
                    write!(cq, ",type={}", escape_cq_value(t)).unwrap();
                }
                if let Some(ref c) = data.cache {
                    write!(cq, ",cache={c}").unwrap();
                }
                if let Some(ref p) = data.proxy {
                    write!(cq, ",proxy={p}").unwrap();
                }
                if let Some(ref t) = data.timeout {
                    write!(cq, ",timeout={t}").unwrap();
                }
                cq.push(']');
                cq
            }
            Segment::Record(data) => {
                let mut cq = format!("[CQ:record,file={}", escape_cq_value(&data.file));
                if let Some(ref m) = data.magic {
                    write!(cq, ",magic={m}").unwrap();
                }
                if let Some(ref c) = data.cache {
                    write!(cq, ",cache={c}").unwrap();
                }
                if let Some(ref p) = data.proxy {
                    write!(cq, ",proxy={p}").unwrap();
                }
                if let Some(ref t) = data.timeout {
                    write!(cq, ",timeout={t}").unwrap();
                }
                cq.push(']');
                cq
            }
            Segment::Video(data) => {
                let mut cq = format!("[CQ:video,file={}", escape_cq_value(&data.file));
                if let Some(ref c) = data.cache {
                    write!(cq, ",cache={c}").unwrap();
                }
                if let Some(ref p) = data.proxy {
                    write!(cq, ",proxy={p}").unwrap();
                }
                if let Some(ref t) = data.timeout {
                    write!(cq, ",timeout={t}").unwrap();
                }
                cq.push(']');
                cq
            }
            Segment::At(data) => format!("[CQ:at,qq={}]", data.qq),
            Segment::Rps(_) => "[CQ:rps]".to_string(),
            Segment::Dice(_) => "[CQ:dice]".to_string(),
            Segment::Shake(_) => "[CQ:shake]".to_string(),
            Segment::Poke(data) => {
                format!("[CQ:poke,type={},id={}]", data.poke_type, data.id)
            }
            Segment::Share(data) => {
                let mut cq = format!(
                    "[CQ:share,url={},title={}",
                    escape_cq_value(&data.url),
                    escape_cq_value(&data.title)
                );
                if let Some(ref c) = data.content {
                    write!(cq, ",content={}", escape_cq_value(c)).unwrap();
                }
                if let Some(ref i) = data.image {
                    write!(cq, ",image={}", escape_cq_value(i)).unwrap();
                }
                cq.push(']');
                cq
            }
            Segment::Contact(data) => {
                format!("[CQ:contact,type={},id={}]", data.contact_type, data.id)
            }
            Segment::Location(data) => {
                let mut cq = format!("[CQ:location,lat={},lon={}", data.lat, data.lon);
                if let Some(ref t) = data.title {
                    write!(cq, ",title={}", escape_cq_value(t)).unwrap();
                }
                if let Some(ref c) = data.content {
                    write!(cq, ",content={}", escape_cq_value(c)).unwrap();
                }
                cq.push(']');
                cq
            }
            Segment::Music(data) => {
                if data.music_type == "custom" {
                    let mut cq = "[CQ:music,type=custom".to_string();
                    if let Some(ref u) = data.url {
                        write!(cq, ",url={}", escape_cq_value(u)).unwrap();
                    }
                    if let Some(ref a) = data.audio {
                        write!(cq, ",audio={}", escape_cq_value(a)).unwrap();
                    }
                    if let Some(ref t) = data.title {
                        write!(cq, ",title={}", escape_cq_value(t)).unwrap();
                    }
                    if let Some(ref c) = data.content {
                        write!(cq, ",content={}", escape_cq_value(c)).unwrap();
                    }
                    if let Some(ref i) = data.image {
                        write!(cq, ",image={}", escape_cq_value(i)).unwrap();
                    }
                    cq.push(']');
                    cq
                } else {
                    format!(
                        "[CQ:music,type={},id={}]",
                        data.music_type,
                        data.id.as_deref().unwrap_or("")
                    )
                }
            }
            Segment::Reply(data) => format!("[CQ:reply,id={}]", data.id),
            Segment::Forward(data) => format!("[CQ:forward,id={}]", data.id),
            Segment::Node(data) => {
                if let Some(ref id) = data.id {
                    format!("[CQ:node,id={id}]")
                } else {
                    let mut cq = "[CQ:node".to_string();
                    if let Some(ref u) = data.user_id {
                        write!(cq, ",user_id={u}").unwrap();
                    }
                    if let Some(ref n) = data.nickname {
                        write!(cq, ",nickname={}", escape_cq_value(n)).unwrap();
                    }
                    if let Some(ref c) = data.content {
                        write!(cq, ",content={}", escape_cq_value(c)).unwrap();
                    }
                    cq.push(']');
                    cq
                }
            }
            Segment::Xml(data) => format!("[CQ:xml,data={}]", escape_cq_value(&data.data)),
            Segment::Json(data) => format!("[CQ:json,data={}]", escape_cq_value(&data.data)),
        }
    }
}

/// Escapes special characters in plain text for CQ code format.
///
/// Escapes: `&` -> `&amp;`, `[` -> `&#91;`, `]` -> `&#93;`
pub fn escape_cq_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
}

/// Unescapes CQ code special characters back to plain text.
pub fn unescape_cq_text(text: &str) -> String {
    text.replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&#44;", ",")
        .replace("&amp;", "&")
}

/// Escapes special characters in CQ code parameter values.
///
/// Escapes: `&` -> `&amp;`, `[` -> `&#91;`, `]` -> `&#93;`, `,` -> `&#44;`
pub fn escape_cq_value(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace(',', "&#44;")
}

/// Unescapes CQ code parameter value special characters.
pub fn unescape_cq_value(value: &str) -> String {
    unescape_cq_text(value)
}

#[cfg(test)]
mod tests {
    use crate::model::message::{OneBotMessage, OneBotMessageExt};
    use crate::model::segment::TextData;

    use super::*;

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
    fn test_parse_cq_escaping() {
        let segments = parse_cq_string("&#91;escaped&#93; &amp; test");
        assert_eq!(segments.len(), 1);
        assert!(matches!(
            &segments[0],
            Segment::Text(TextData { text }) if text == "[escaped] & test"
        ));
    }

    #[test]
    fn test_segment_cq_code_conversion() {
        assert_eq!(Segment::text("Hello").to_cq_code(), "Hello");
        assert_eq!(Segment::face(178).to_cq_code(), "[CQ:face,id=178]");
        assert_eq!(Segment::at(10001000).to_cq_code(), "[CQ:at,qq=10001000]");
        assert_eq!(Segment::at_all().to_cq_code(), "[CQ:at,qq=all]");
        assert_eq!(Segment::rps().to_cq_code(), "[CQ:rps]");
        assert_eq!(Segment::dice().to_cq_code(), "[CQ:dice]");
        assert_eq!(
            Segment::image("http://example.com/1.jpg").to_cq_code(),
            "[CQ:image,file=http://example.com/1.jpg]"
        );
    }

    #[test]
    fn test_segment_cq_escaping() {
        assert_eq!(escape_cq_text("Hello [World]"), "Hello &#91;World&#93;");
        assert_eq!(escape_cq_text("A & B"), "A &amp; B");
        assert_eq!(unescape_cq_text("&#91;x&#93; &amp;"), "[x] &");

        assert_eq!(escape_cq_value("a,b,c"), "a&#44;b&#44;c");
        assert_eq!(unescape_cq_value("a&#44;b&#44;c"), "a,b,c");
    }
}
