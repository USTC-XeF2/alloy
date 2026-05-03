use std::borrow::Cow;

use amira_core::{ReceiveMessageSegment, RichTextSegment, SendMessageSegment};
use serde::Serialize;

use super::common::{FaceData, MentionAllData, TextData};

/// A Milky protocol message segment **sent** to the server.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum OutgoingSegment {
    /// Plain text.
    Text(TextData),
    /// @mention a specific user.
    Mention(MentionData),
    /// @everyone in a group.
    MentionAll(MentionAllData),
    /// QQ emoji/face.
    Face(FaceData),
    /// Reply-quote to an earlier message.
    Reply(ReplyData),
    /// Image.
    Image(ImageData),
    /// Voice/audio recording.
    Record(RecordData),
    /// Video.
    Video(VideoData),
    /// Forwarded message bundle.
    Forward(ForwardData),
    /// Mini program / light app card.
    #[cfg(feature = "v1_2")]
    LightApp(LightAppData),
}

impl std::fmt::Display for OutgoingSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutgoingSegment::Text(d) => write!(f, "{}", d.text),
            OutgoingSegment::Mention(d) => write!(f, "@{}", d.user_id),
            OutgoingSegment::MentionAll(_) => write!(f, "@全体成员"),
            OutgoingSegment::Face(d) => write!(f, "[表情:{}]", d.face_id),
            OutgoingSegment::Reply(d) => write!(f, "[回复:{}]", d.message_seq),
            OutgoingSegment::Image(_) => write!(f, "[图片]"),
            OutgoingSegment::Record(_) => write!(f, "[语音]"),
            OutgoingSegment::Video(_) => write!(f, "[视频]"),
            OutgoingSegment::Forward(_) => write!(f, "[合并转发]"),
            #[cfg(feature = "v1_2")]
            OutgoingSegment::LightApp(_) => write!(f, "[小程序]"),
        }
    }
}

impl ReceiveMessageSegment for OutgoingSegment {
    fn segment_type(&self) -> &'static str {
        match self {
            OutgoingSegment::Text(_) => "text",
            OutgoingSegment::Mention(_) => "mention",
            OutgoingSegment::MentionAll(_) => "mention_all",
            OutgoingSegment::Face(_) => "face",
            OutgoingSegment::Reply(_) => "reply",
            OutgoingSegment::Image(_) => "image",
            OutgoingSegment::Record(_) => "record",
            OutgoingSegment::Video(_) => "video",
            OutgoingSegment::Forward(_) => "forward",
            #[cfg(feature = "v1_2")]
            OutgoingSegment::LightApp(_) => "light_app",
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            OutgoingSegment::Text(d) => Some(&d.text),
            _ => None,
        }
    }

    fn as_rich_text(&self) -> Option<RichTextSegment> {
        match self {
            OutgoingSegment::Text(d) => Some(RichTextSegment::Text(d.text.clone())),
            OutgoingSegment::Image(d) => Some(RichTextSegment::Image(d.uri.clone())),
            OutgoingSegment::Mention(d) => Some(RichTextSegment::At(d.user_id.to_string())),
            OutgoingSegment::MentionAll(_) => Some(RichTextSegment::AtAll),
            OutgoingSegment::Reply(d) => Some(RichTextSegment::Reply(d.message_seq.to_string())),
            _ => None,
        }
    }
}

impl SendMessageSegment for OutgoingSegment {
    fn from_rich_text_segment(seg: RichTextSegment) -> Option<Self> {
        match seg {
            RichTextSegment::Text(s) => Some(Self::text(s)),
            RichTextSegment::Image(r) => Some(Self::image(r)),
            RichTextSegment::At(id) => id.parse::<i64>().ok().map(Self::mention),
            RichTextSegment::AtAll => Some(Self::mention_all()),
            RichTextSegment::Reply(id) => id.parse::<i64>().ok().map(Self::reply),
        }
    }
}

impl OutgoingSegment {
    /// Creates a text segment.
    pub fn text(text: impl Into<Cow<'static, str>>) -> Self {
        OutgoingSegment::Text(TextData { text: text.into() })
    }

    /// Creates a mention segment for a specific user.
    pub fn mention(user_id: i64) -> Self {
        OutgoingSegment::Mention(MentionData { user_id })
    }

    /// Creates a mention-all segment.
    pub fn mention_all() -> Self {
        OutgoingSegment::MentionAll(MentionAllData {})
    }

    /// Creates a face/emoji segment.
    pub fn face(face_id: impl Into<String>, #[cfg(feature = "v1_1")] is_large: bool) -> Self {
        OutgoingSegment::Face(FaceData {
            face_id: face_id.into(),
            #[cfg(feature = "v1_1")]
            is_large,
        })
    }

    /// Creates a reply segment referencing a message by sequence number.
    pub fn reply(message_seq: i64) -> Self {
        OutgoingSegment::Reply(ReplyData { message_seq })
    }

    /// Creates an image segment from a URI (`file://`, `http(s)://`, `base64://`).
    pub fn image(uri: impl Into<Cow<'static, str>>) -> Self {
        OutgoingSegment::Image(ImageData {
            uri: uri.into(),
            sub_type: ImageSubType::Normal,
            summary: None,
        })
    }

    /// Creates a sticker (animated image) segment.
    pub fn sticker(uri: impl Into<Cow<'static, str>>) -> Self {
        OutgoingSegment::Image(ImageData {
            uri: uri.into(),
            sub_type: ImageSubType::Sticker,
            summary: None,
        })
    }

    /// Creates a voice/record segment.
    pub fn record(uri: impl Into<String>) -> Self {
        OutgoingSegment::Record(RecordData { uri: uri.into() })
    }

    /// Creates a video segment.
    pub fn video(uri: impl Into<String>) -> Self {
        OutgoingSegment::Video(VideoData {
            uri: uri.into(),
            thumb_uri: None,
        })
    }

    /// Creates a merged-forward bundle segment from a list of message nodes.
    pub fn forward(messages: Vec<ForwardedMessage>) -> Self {
        OutgoingSegment::Forward(ForwardData {
            messages,
            #[cfg(feature = "v1_2")]
            title: None,
            #[cfg(feature = "v1_2")]
            preview: None,
            #[cfg(feature = "v1_2")]
            summary: None,
            #[cfg(feature = "v1_2")]
            prompt: None,
        })
    }

    /// Creates a light-app (mini program) segment from a JSON payload.
    #[cfg(feature = "v1_2")]
    pub fn light_app(json_payload: impl Into<String>) -> Self {
        OutgoingSegment::LightApp(LightAppData {
            json_payload: json_payload.into(),
        })
    }
}

/// Outgoing mention data - just the user ID, no display name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MentionData {
    pub user_id: i64,
}

/// Outgoing reply data - only the referenced message sequence number.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplyData {
    pub message_seq: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSubType {
    Normal,
    Sticker,
}

/// Outgoing image data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImageData {
    /// File URI (`file://`, `http(s)://`, `base64://`).
    pub uri: Cow<'static, str>,
    pub sub_type: ImageSubType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Outgoing record (voice) data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordData {
    pub uri: String,
}

/// Outgoing video data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoData {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_uri: Option<String>,
}

/// Outgoing merged-forward bundle data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForwardData {
    /// Message nodes to include (required).
    pub messages: Vec<ForwardedMessage>,
    #[cfg(feature = "v1_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Preview lines (1-4).
    #[cfg(feature = "v1_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Vec<String>>,
    #[cfg(feature = "v1_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// External display text on mobile QQ.
    #[cfg(feature = "v1_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// A single message node inside an outgoing forwarded bundle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForwardedMessage {
    pub user_id: i64,
    pub sender_name: String,
    pub segments: Vec<OutgoingSegment>,
}

/// Outgoing light-app (mini program) data - json payload only.
#[cfg(feature = "v1_2")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LightAppData {
    pub json_payload: String,
}
