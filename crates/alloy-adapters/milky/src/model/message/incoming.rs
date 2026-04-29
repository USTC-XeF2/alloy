use alloy_core::{ReceiveMessageSegment, RichTextSegment};
use serde::Deserialize;

use super::common::{FaceData, MentionAllData, TextData};

/// A Milky protocol message segment **received** from the server.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum IncomingSegment {
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
    /// File attachment (incoming only; files are never sent via segments).
    File(FileData),
    /// Forwarded message bundle.
    Forward(ForwardData),
    /// Market / sticker pack emoji (incoming only).
    MarketFace(MarketFaceData),
    /// Mini program / light app card (incoming only).
    LightApp(LightAppData),
    /// Raw XML message card (incoming only).
    Xml(XmlData),
}

impl std::fmt::Display for IncomingSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IncomingSegment::Text(d) => write!(f, "{}", d.text),
            #[cfg(feature = "v1_2")]
            IncomingSegment::Mention(d) => write!(f, "@{}", d.name),
            #[cfg(not(feature = "v1_2"))]
            IncomingSegment::Mention(d) => write!(f, "@{}", d.user_id),
            IncomingSegment::MentionAll(_) => write!(f, "@全体成员"),
            IncomingSegment::Face(d) => write!(f, "[表情:{}]", d.face_id),
            IncomingSegment::Reply(d) => write!(f, "[回复:{}]", d.message_seq),
            IncomingSegment::Image(_) => write!(f, "[图片]"),
            IncomingSegment::Record(_) => write!(f, "[语音]"),
            IncomingSegment::Video(_) => write!(f, "[视频]"),
            IncomingSegment::File(d) => write!(f, "[文件:{}]", d.file_name),
            IncomingSegment::Forward(_) => write!(f, "[合并转发]"),
            #[cfg(feature = "v1_1")]
            IncomingSegment::MarketFace(d) => write!(f, "[表情:{}]", d.summary),
            #[cfg(not(feature = "v1_1"))]
            IncomingSegment::MarketFace(_) => write!(f, "[表情]"),
            IncomingSegment::LightApp(d) => write!(f, "[小程序:{}]", d.app_name),
            IncomingSegment::Xml(_) => write!(f, "[XML消息]"),
        }
    }
}

impl ReceiveMessageSegment for IncomingSegment {
    fn segment_type(&self) -> &'static str {
        match self {
            IncomingSegment::Text(_) => "text",
            IncomingSegment::Mention(_) => "mention",
            IncomingSegment::MentionAll(_) => "mention_all",
            IncomingSegment::Face(_) => "face",
            IncomingSegment::Reply(_) => "reply",
            IncomingSegment::Image(_) => "image",
            IncomingSegment::Record(_) => "record",
            IncomingSegment::Video(_) => "video",
            IncomingSegment::File(_) => "file",
            IncomingSegment::Forward(_) => "forward",
            IncomingSegment::MarketFace(_) => "market_face",
            IncomingSegment::LightApp(_) => "light_app",
            IncomingSegment::Xml(_) => "xml",
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            IncomingSegment::Text(d) => Some(&d.text),
            _ => None,
        }
    }

    fn as_rich_text(&self) -> Option<RichTextSegment> {
        match self {
            IncomingSegment::Text(d) => Some(RichTextSegment::Text(d.text.clone())),
            IncomingSegment::Image(d) => Some(RichTextSegment::Image(d.temp_url.clone().into())),
            IncomingSegment::Mention(d) => Some(RichTextSegment::At(d.user_id.to_string())),
            IncomingSegment::MentionAll(_) => Some(RichTextSegment::AtAll),
            _ => None,
        }
    }
}

/// Incoming mention data - includes the display name alongside the user ID.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MentionData {
    pub user_id: i64,
    /// Mentioned user's display name (without the `@` prefix).
    #[cfg(feature = "v1_2")]
    pub name: String,
}

/// Incoming reply-quote data - includes the full quoted message content.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReplyData {
    pub message_seq: i64,
    #[cfg(feature = "v1_2")]
    pub sender_id: i64,
    /// Sender name - only available inside a merged-forward context.
    #[cfg(feature = "v1_2")]
    #[serde(default)]
    pub sender_name: Option<String>,
    #[cfg(feature = "v1_2")]
    pub time: i64,
    /// Segments of the quoted message.
    #[cfg(feature = "v1_2")]
    pub segments: Vec<IncomingSegment>,
}

/// Incoming image data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImageData {
    pub resource_id: String,
    pub temp_url: String,
    pub width: i32,
    pub height: i32,
    pub summary: String,
    /// `"normal"` or `"sticker"`.
    pub sub_type: String,
}

/// Incoming record (voice) data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecordData {
    pub resource_id: String,
    pub temp_url: String,
    /// Duration in seconds.
    pub duration: i32,
}

/// Incoming video data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoData {
    pub resource_id: String,
    pub temp_url: String,
    pub width: i32,
    pub height: i32,
    /// Duration in seconds.
    pub duration: i32,
}

/// File attachment data (incoming only).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FileData {
    pub file_id: String,
    pub file_name: String,
    pub file_size: i64,
    /// TriSHA1 hash - only present in private-chat files.
    #[serde(default)]
    pub file_hash: Option<String>,
}

/// Incoming merged-forward bundle metadata.
///
/// To retrieve the actual messages, call `get_forwarded_messages` with `forward_id`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ForwardData {
    pub forward_id: String,
    #[cfg(feature = "v1_1")]
    pub title: String,
    #[cfg(feature = "v1_1")]
    pub preview: Vec<String>,
    #[cfg(feature = "v1_1")]
    pub summary: String,
}

/// Market / sticker-pack emoji data (incoming only).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MarketFaceData {
    #[cfg(feature = "v1_1")]
    pub emoji_package_id: i32,
    #[cfg(feature = "v1_1")]
    pub emoji_id: String,
    #[cfg(feature = "v1_1")]
    pub key: String,
    #[cfg(feature = "v1_1")]
    pub summary: String,
    pub url: String,
}

/// Light-app (mini program) card data (incoming).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LightAppData {
    pub app_name: String,
    pub json_payload: String,
}

/// XML message card data (incoming only).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct XmlData {
    pub service_id: i32,
    pub xml_payload: String,
}
