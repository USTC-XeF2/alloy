use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Plain text segment data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextData {
    pub text: Cow<'static, str>,
}

/// Mention-all segment data (empty struct).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionAllData {}

/// Face/emoji segment data - same structure in both directions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceData {
    pub face_id: String,
    #[cfg(feature = "v1_1")]
    pub is_large: bool,
}
