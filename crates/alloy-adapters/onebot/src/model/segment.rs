//! OneBot v11 Message Segment types.
//!
//! This module defines all message segment types as specified in the OneBot v11 protocol.
//! A message segment represents a single unit of content in a message, such as plain text,
//! images, mentions, etc.
//!
//! # CQ Code Mapping
//!
//! Each segment type corresponds to a CQ code in the string format:
//! - `text` → plain text (no CQ code)
//! - `face` → `[CQ:face,id=123]`
//! - `image` → `[CQ:image,file=xxx]`
//! - etc.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::Segment;
//!
//! let text = Segment::text("Hello, ");
//! let at = Segment::at(10001000);
//! let face = Segment::face(178);
//! ```

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use alloy_core::{MessageSegment as MessageSegmentTrait, RichTextSegment};

// ============================================================================
// Segment Enum - The main message segment type
// ============================================================================

/// A OneBot v11 message segment.
///
/// This enum represents all possible segment types in the `OneBot` v11 protocol.
/// Each variant contains the specific data for that segment type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Segment {
    /// Plain text content.
    Text(TextData),
    /// QQ emoji/face.
    Face(FaceData),
    /// Image.
    Image(ImageData),
    /// Voice/Audio record.
    Record(RecordData),
    /// Video.
    Video(VideoData),
    /// @mention someone.
    At(AtData),
    /// Rock-paper-scissors magic emoji.
    Rps(RpsData),
    /// Dice magic emoji.
    Dice(DiceData),
    /// Window shake (legacy poke).
    Shake(ShakeData),
    /// Poke message.
    Poke(PokeData),
    /// Anonymous flag (send only).
    Anonymous(AnonymousData),
    /// Link share.
    Share(ShareData),
    /// Contact recommendation.
    Contact(ContactData),
    /// Location.
    Location(LocationData),
    /// Music share.
    Music(MusicData),
    /// Reply to a message.
    Reply(ReplyData),
    /// Forward message reference (receive only).
    Forward(ForwardData),
    /// Forward node (for constructing forward messages).
    Node(NodeData),
    /// XML message.
    Xml(XmlData),
    /// JSON message.
    Json(JsonData),
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Segment::Text(data) => write!(f, "{}", data.text),
            Segment::Face(data) => write!(f, "[表情:{}]", data.id),
            Segment::Image(data) => write!(f, "[图片:{}]", data.file),
            Segment::Record(data) => write!(f, "[语音:{}]", data.file),
            Segment::Video(data) => write!(f, "[视频:{}]", data.file),
            Segment::At(data) => {
                if data.qq == "all" {
                    write!(f, "@全体成员")
                } else {
                    write!(f, "@{}", data.qq)
                }
            }
            Segment::Rps(_) => write!(f, "[猜拳]"),
            Segment::Dice(_) => write!(f, "[骰子]"),
            Segment::Shake(_) => write!(f, "[窗口抖动]"),
            Segment::Poke(data) => write!(f, "[戳一戳:{}]", data.poke_type),
            Segment::Anonymous(_) => write!(f, "[匿名]"),
            Segment::Share(data) => write!(f, "[分享:{}]", data.title),
            Segment::Contact(data) => write!(f, "[推荐{}:{}]", data.contact_type, data.id),
            Segment::Location(data) => {
                write!(f, "[位置:{},{}]", data.lat, data.lon)
            }
            Segment::Music(data) => write!(f, "[音乐:{}]", data.music_type),
            Segment::Reply(data) => write!(f, "[回复:{}]", data.id),
            Segment::Forward(data) => write!(f, "[合并转发:{}]", data.id),
            Segment::Node(_) => write!(f, "[转发节点]"),
            Segment::Xml(_) => write!(f, "[XML消息]"),
            Segment::Json(_) => write!(f, "[JSON消息]"),
        }
    }
}

impl MessageSegmentTrait for Segment {
    fn text(text: impl Into<String>) -> Self {
        Segment::Text(TextData { text: text.into() })
    }

    fn segment_type(&self) -> &str {
        match self {
            Segment::Text(_) => "text",
            Segment::Face(_) => "face",
            Segment::Image(_) => "image",
            Segment::Record(_) => "record",
            Segment::Video(_) => "video",
            Segment::At(_) => "at",
            Segment::Rps(_) => "rps",
            Segment::Dice(_) => "dice",
            Segment::Shake(_) => "shake",
            Segment::Poke(_) => "poke",
            Segment::Anonymous(_) => "anonymous",
            Segment::Share(_) => "share",
            Segment::Contact(_) => "contact",
            Segment::Location(_) => "location",
            Segment::Music(_) => "music",
            Segment::Reply(_) => "reply",
            Segment::Forward(_) => "forward",
            Segment::Node(_) => "node",
            Segment::Xml(_) => "xml",
            Segment::Json(_) => "json",
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Segment::Text(data) => Some(&data.text),
            _ => None,
        }
    }

    fn as_rich_text(&self) -> Option<RichTextSegment> {
        match self {
            Segment::Text(data) => Some(RichTextSegment::Text(data.text.clone())),
            Segment::Image(data) => Some(RichTextSegment::Image(data.file.clone())),
            Segment::At(data) => Some(RichTextSegment::At(data.qq.clone())),
            _ => None,
        }
    }

    fn from_rich_text_segment(seg: &RichTextSegment) -> Option<Self> {
        match seg {
            RichTextSegment::Text(s) => Some(Segment::text(s)),
            RichTextSegment::Image(r) => Some(Segment::image(r)),
            RichTextSegment::At(id) => Some(Segment::At(AtData { qq: id.clone() })),
        }
    }
}

// ============================================================================
// Segment Builder Methods
// ============================================================================

impl Segment {
    // --------------------------------
    // Face
    // --------------------------------

    /// Creates a QQ face/emoji segment.
    pub fn face(id: i32) -> Self {
        Segment::Face(FaceData { id: id.to_string() })
    }

    // --------------------------------
    // Image
    // --------------------------------

    /// Creates an image segment from a file path or URL.
    pub fn image(file: impl Into<String>) -> Self {
        Segment::Image(ImageData {
            file: file.into(),
            image_type: None,
            url: None,
            cache: None,
            proxy: None,
            timeout: None,
        })
    }

    /// Creates a flash image segment.
    pub fn flash_image(file: impl Into<String>) -> Self {
        Segment::Image(ImageData {
            file: file.into(),
            image_type: Some("flash".to_string()),
            url: None,
            cache: None,
            proxy: None,
            timeout: None,
        })
    }

    // --------------------------------
    // Record
    // --------------------------------

    /// Creates a voice/record segment.
    pub fn record(file: impl Into<String>) -> Self {
        Segment::Record(RecordData {
            file: file.into(),
            magic: None,
            url: None,
            cache: None,
            proxy: None,
            timeout: None,
        })
    }

    // --------------------------------
    // Video
    // --------------------------------

    /// Creates a video segment.
    pub fn video(file: impl Into<String>) -> Self {
        Segment::Video(VideoData {
            file: file.into(),
            url: None,
            cache: None,
            proxy: None,
            timeout: None,
        })
    }

    // --------------------------------
    // At
    // --------------------------------

    /// Creates an @mention segment for a specific user.
    pub fn at(qq: i64) -> Self {
        Segment::At(AtData { qq: qq.to_string() })
    }

    /// Creates an @all segment to mention everyone.
    pub fn at_all() -> Self {
        Segment::At(AtData {
            qq: "all".to_string(),
        })
    }

    // --------------------------------
    // Magic Emojis
    // --------------------------------

    /// Creates a rock-paper-scissors segment.
    pub fn rps() -> Self {
        Segment::Rps(RpsData {})
    }

    /// Creates a dice segment.
    pub fn dice() -> Self {
        Segment::Dice(DiceData {})
    }

    /// Creates a shake (legacy poke) segment.
    pub fn shake() -> Self {
        Segment::Shake(ShakeData {})
    }

    // --------------------------------
    // Poke
    // --------------------------------

    /// Creates a poke segment.
    pub fn poke(poke_type: impl Into<String>, id: impl Into<String>) -> Self {
        Segment::Poke(PokeData {
            poke_type: poke_type.into(),
            id: id.into(),
            name: None,
        })
    }

    // --------------------------------
    // Share
    // --------------------------------

    /// Creates a link share segment.
    pub fn share(url: impl Into<String>, title: impl Into<String>) -> Self {
        Segment::Share(ShareData {
            url: url.into(),
            title: title.into(),
            content: None,
            image: None,
        })
    }

    // --------------------------------
    // Contact
    // --------------------------------

    /// Creates a friend recommendation segment.
    pub fn contact_qq(id: i64) -> Self {
        Segment::Contact(ContactData {
            contact_type: "qq".to_string(),
            id: id.to_string(),
        })
    }

    /// Creates a group recommendation segment.
    pub fn contact_group(id: i64) -> Self {
        Segment::Contact(ContactData {
            contact_type: "group".to_string(),
            id: id.to_string(),
        })
    }

    // --------------------------------
    // Location
    // --------------------------------

    /// Creates a location segment.
    pub fn location(lat: f64, lon: f64) -> Self {
        Segment::Location(LocationData {
            lat: lat.to_string(),
            lon: lon.to_string(),
            title: None,
            content: None,
        })
    }

    // --------------------------------
    // Music
    // --------------------------------

    /// Creates a music share segment (QQ Music, NetEase, etc.).
    pub fn music(music_type: impl Into<String>, id: impl Into<String>) -> Self {
        Segment::Music(MusicData {
            music_type: music_type.into(),
            id: Some(id.into()),
            url: None,
            audio: None,
            title: None,
            content: None,
            image: None,
        })
    }

    /// Creates a custom music share segment.
    pub fn music_custom(
        url: impl Into<String>,
        audio: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Segment::Music(MusicData {
            music_type: "custom".to_string(),
            id: None,
            url: Some(url.into()),
            audio: Some(audio.into()),
            title: Some(title.into()),
            content: None,
            image: None,
        })
    }

    // --------------------------------
    // Reply
    // --------------------------------

    /// Creates a reply segment referencing another message.
    pub fn reply(id: impl Into<String>) -> Self {
        Segment::Reply(ReplyData { id: id.into() })
    }

    // --------------------------------
    // Forward
    // --------------------------------

    /// Creates a forward reference segment (for receiving).
    pub fn forward(id: impl Into<String>) -> Self {
        Segment::Forward(ForwardData { id: id.into() })
    }

    /// Creates a forward node segment referencing an existing message.
    pub fn node(id: impl Into<String>) -> Self {
        Segment::Node(NodeData {
            id: Some(id.into()),
            user_id: None,
            nickname: None,
            content: None,
        })
    }

    /// Creates a custom forward node segment.
    pub fn node_custom(
        user_id: i64,
        nickname: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Segment::Node(NodeData {
            id: None,
            user_id: Some(user_id.to_string()),
            nickname: Some(nickname.into()),
            content: Some(content.into()),
        })
    }

    // --------------------------------
    // XML/JSON
    // --------------------------------

    /// Creates an XML message segment.
    pub fn xml(data: impl Into<String>) -> Self {
        Segment::Xml(XmlData { data: data.into() })
    }

    /// Creates a JSON message segment.
    pub fn json(data: impl Into<String>) -> Self {
        Segment::Json(JsonData { data: data.into() })
    }
}

// ============================================================================
// Segment Data Types
// ============================================================================

/// Plain text segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextData {
    /// The text content.
    pub text: String,
}

/// QQ face/emoji segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceData {
    /// The face ID. See QQ face ID table.
    pub id: String,
}

/// Image segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    /// Image file name, path, URL, or base64.
    pub file: String,
    /// Image type: "flash" for flash image, None for normal.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub image_type: Option<String>,
    /// Image URL (receive only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether to use cached file (send only, default: 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    /// Whether to use proxy (send only, default: 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Download timeout in seconds (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Voice/Record segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordData {
    /// Audio file name, path, URL, or base64.
    pub file: String,
    /// Voice change: "0" or "1".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic: Option<String>,
    /// Audio URL (receive only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether to use cached file (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    /// Whether to use proxy (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Download timeout in seconds (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Video segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoData {
    /// Video file name, path, URL, or base64.
    pub file: String,
    /// Video URL (receive only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether to use cached file (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    /// Whether to use proxy (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Download timeout in seconds (send only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// @mention segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtData {
    /// QQ number or "all" for @everyone.
    pub qq: String,
}

/// Rock-paper-scissors segment data (empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpsData {}

/// Dice segment data (empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiceData {}

/// Shake segment data (empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShakeData {}

/// Poke segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PokeData {
    /// Poke type. See Mirai's PokeMessage.
    #[serde(rename = "type")]
    pub poke_type: String,
    /// Poke ID.
    pub id: String,
    /// Poke name (receive only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Anonymous segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnonymousData {
    /// Whether to continue sending if anonymous fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<String>,
}

/// Link share segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareData {
    /// Share URL.
    pub url: String,
    /// Share title.
    pub title: String,
    /// Share content/description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Share image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

/// Contact recommendation segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContactData {
    /// Contact type: "qq" or "group".
    #[serde(rename = "type")]
    pub contact_type: String,
    /// QQ number or group ID.
    pub id: String,
}

/// Location segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationData {
    /// Latitude.
    pub lat: String,
    /// Longitude.
    pub lon: String,
    /// Location title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Location content/description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Music share segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicData {
    /// Music type: "qq", "163", "xm", or "custom".
    #[serde(rename = "type")]
    pub music_type: String,
    /// Music ID (for qq/163/xm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Click URL (for custom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Audio URL (for custom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    /// Music title (for custom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Music content/description (for custom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Music cover image URL (for custom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

/// Reply segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplyData {
    /// Message ID to reply to.
    pub id: String,
}

/// Forward reference segment data (receive only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForwardData {
    /// Forward message ID.
    pub id: String,
}

/// Forward node segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    /// Reference an existing message by ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Custom node: sender user ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Custom node: sender nickname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// Custom node: message content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// XML message segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XmlData {
    /// XML content.
    pub data: String,
}

/// JSON message segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonData {
    /// JSON content.
    pub data: String,
}

// ============================================================================
// CQ Code Conversion
// ============================================================================

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
            Segment::Anonymous(data) => {
                if let Some(ref i) = data.ignore {
                    format!("[CQ:anonymous,ignore={i}]")
                } else {
                    "[CQ:anonymous]".to_string()
                }
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

// ============================================================================
// CQ Code Escaping Utilities
// ============================================================================

/// Escapes special characters in plain text for CQ code format.
///
/// Escapes: `&` → `&amp;`, `[` → `&#91;`, `]` → `&#93;`
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
/// Escapes: `&` → `&amp;`, `[` → `&#91;`, `]` → `&#93;`, `,` → `&#44;`
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_serialize() {
        let text = Segment::text("Hello");
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, r#"{"type":"text","data":{"text":"Hello"}}"#);

        let at = Segment::at(10001000);
        let json = serde_json::to_string(&at).unwrap();
        assert_eq!(json, r#"{"type":"at","data":{"qq":"10001000"}}"#);

        let face = Segment::face(178);
        let json = serde_json::to_string(&face).unwrap();
        assert_eq!(json, r#"{"type":"face","data":{"id":"178"}}"#);
    }

    #[test]
    fn test_segment_deserialize() {
        let json = r#"{"type":"text","data":{"text":"Hello World"}}"#;
        let segment: Segment = serde_json::from_str(json).unwrap();
        assert!(matches!(segment, Segment::Text(TextData { text }) if text == "Hello World"));

        let json =
            r#"{"type":"image","data":{"file":"123.jpg","url":"http://example.com/123.jpg"}}"#;
        let segment: Segment = serde_json::from_str(json).unwrap();
        assert!(
            matches!(segment, Segment::Image(ImageData { file, url: Some(_), .. }) if file == "123.jpg")
        );

        let json = r#"{"type":"at","data":{"qq":"all"}}"#;
        let segment: Segment = serde_json::from_str(json).unwrap();
        assert!(matches!(segment, Segment::At(AtData { qq }) if qq == "all"));
    }

    #[test]
    fn test_cq_code_conversion() {
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
    fn test_cq_escaping() {
        assert_eq!(escape_cq_text("Hello [World]"), "Hello &#91;World&#93;");
        assert_eq!(escape_cq_text("A & B"), "A &amp; B");
        assert_eq!(unescape_cq_text("&#91;x&#93; &amp;"), "[x] &");

        assert_eq!(escape_cq_value("a,b,c"), "a&#44;b&#44;c");
        assert_eq!(unescape_cq_value("a&#44;b&#44;c"), "a,b,c");
    }

    #[test]
    fn test_message_segment_trait() {
        let text = Segment::text("Hello");
        assert_eq!(text.segment_type(), "text");
        assert!(text.is_text());
        assert_eq!(text.as_text(), Some("Hello"));

        let image = Segment::image("test.jpg");
        assert_eq!(image.segment_type(), "image");
        assert!(!image.is_text());
        assert_eq!(image.as_text(), None);
    }
}
