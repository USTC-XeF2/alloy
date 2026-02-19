use std::collections::HashMap;

/// Prefix used for image placeholder tokens in command argument strings.
pub const IMAGE_PLACEHOLDER_PREFIX: &str = "\x00IMG_";

/// Prefix used for at-mention placeholder tokens in command argument strings.
pub const AT_PLACEHOLDER_PREFIX: &str = "\x00AT_";

/// A shared registry mapping placeholder tokens to their original values.
///
/// Stored in [`AlloyContext`] so that `ImageSegment` and `AtSegment` can
/// look up their real data after clap parsing.
#[derive(Clone, Debug, Default)]
pub struct HandleRegistry {
    pub images: HashMap<String, String>,
    pub ats: HashMap<String, String>,
}

/// A segment containing an image that appeared in a command argument.
///
/// During parsing, image segments in the message are replaced by opaque
/// placeholder tokens. `ImageSegment` stores that token and can resolve
/// it back to the original image reference (file path, URL, base64, etc.).
///
/// # Usage
///
/// Use `ImageSegment` as a field type in your clap `Parser` struct.
/// It dereferences to `&str` for easy access to the image reference:
///
/// ```rust,ignore
/// #[derive(Parser, Clone)]
/// struct MyCommand {
///     img: ImageSegment,
/// }
///
/// async fn handler(cmd: CommandArgs<MyCommand>) {
///     let image_ref: &str = &cmd.img;  // Deref to &str
///     println!("Image: {}", image_ref);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ImageSegment {
    /// The original image reference resolved from the registry.
    value: String,
}

impl std::ops::Deref for ImageSegment {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for ImageSegment {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for ImageSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl std::str::FromStr for ImageSegment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Resolve from thread-local registry set during parsing.
        super::CURRENT_REGISTRY.with(|reg| {
            reg.borrow()
                .as_ref()
                .and_then(|r| r.images.get(s).cloned())
                .map(|value| ImageSegment { value })
                .ok_or_else(|| format!("not a valid image segment: {s}"))
        })
    }
}

/// A segment containing an at-mention that appeared in a command argument.
///
/// During parsing, at-mention segments in the message are replaced by opaque
/// placeholder tokens. `AtSegment` stores that token and can resolve it back
/// to the original user identifier.
///
/// # Usage
///
/// Use `AtSegment` as a field type in your clap `Parser` struct.
/// It dereferences to `&str` for easy access to the user identifier:
///
/// ```rust,ignore
/// #[derive(Parser, Clone)]
/// struct MyCommand {
///     target: AtSegment,
/// }
///
/// async fn handler(cmd: CommandArgs<MyCommand>) {
///     let user_id: &str = &cmd.target;  // Deref to &str
///     println!("User: {}", user_id);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AtSegment {
    /// The original user identifier resolved from the registry.
    value: String,
}

impl std::ops::Deref for AtSegment {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for AtSegment {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for AtSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl std::str::FromStr for AtSegment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Resolve from thread-local registry set during parsing.
        super::CURRENT_REGISTRY.with(|reg| {
            reg.borrow()
                .as_ref()
                .and_then(|r| r.ats.get(s).cloned())
                .map(|value| AtSegment { value })
                .ok_or_else(|| format!("not a valid at segment: {s}"))
        })
    }
}
