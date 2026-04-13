use std::fmt::Write;

use alloy_core::Message;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use super::segment::Segment;

/// Custom deserializer for `Message<Segment>` that supports both array and string formats.
pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Message<Segment>, D::Error>
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
        MessageFormat::Array(segments) => Ok(segments.into()),
        MessageFormat::String(cq_string) => {
            let segments = parse_cq_string(&cq_string);
            Ok(segments.into())
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
            let mut params = Map::new();
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
                    params
                        .entry(param_name)
                        .or_insert_with(|| Value::String(param_value));
                }
            }

            // Skip the closing ]
            if pos < len && chars[pos] == ']' {
                pos += 1;
            }

            // Create segment from parsed CQ code
            let mut raw_segment = Map::new();
            raw_segment.insert("type".to_string(), Value::String(func_name));
            raw_segment.insert("data".to_string(), Value::Object(params));

            if let Ok(segment) = serde_json::from_value(Value::Object(raw_segment)) {
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

impl Segment {
    /// Converts this segment to a CQ code string.
    ///
    /// Text segments are returned as plain text (with escaping).
    /// Other segments are formatted as `[CQ:type,key=value,...]`.
    pub fn to_cq_code(&self) -> String {
        let Ok(Value::Object(obj)) = serde_json::to_value(self) else {
            return String::new();
        };

        let Some(cq_type) = obj.get("type").and_then(Value::as_str) else {
            return String::new();
        };

        let Some(data) = obj.get("data").and_then(Value::as_object) else {
            return String::new();
        };

        if cq_type == "text" {
            return data
                .get("text")
                .and_then(Value::as_str)
                .map(escape_cq_text)
                .unwrap_or_default();
        }

        let mut cq = format!("[CQ:{cq_type}");
        for (key, value) in data {
            let encoded = match value {
                Value::String(s) => escape_cq_value(s),
                _ => escape_cq_value(&value.to_string()),
            };
            write!(cq, ",{key}={encoded}").unwrap();
        }
        cq.push(']');
        cq
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
        let msg = deserialize(&mut serde_json::Deserializer::from_str(json)).unwrap();
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
        let msg = OneBotMessage::from(vec![
            Segment::text("Hello "),
            Segment::face("178"),
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
        assert_eq!(Segment::face("178").to_cq_code(), "[CQ:face,id=178]");
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
