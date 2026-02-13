//! Command parsing system using clap.
//!
//! This module provides a clap-based command parsing system that integrates
//! with the Alloy matcher system. It supports:
//!
//! - Automatic command parsing from message rich text
//! - Type-safe command extraction via `Command<T>` extractor
//! - Help message generation on parse errors
//! - Rich text segments: [`ImageSegment`] and [`AtSegment`] for accessing non-text
//!   segments that appear as command arguments
//!
//! # Rich Text Parsing
//!
//! When a message contains images or mentions mixed with text, the parser
//! replaces them with unique placeholder tokens before shell-splitting.
//! After clap parsing, handlers can use `ImageSegment` and `AtSegment` to
//! retrieve the original rich content:
//!
//! ```rust,ignore
//! use clap::Parser;
//! use alloy_framework::{on_command, CommandArgs, ImageSegment, AtSegment};
//!
//! #[derive(Parser, Clone)]
//! struct SendCommand {
//!     /// The user to send to
//!     target: AtSegment,
//!     /// An image to send
//!     image: ImageSegment,
//! }
//!
//! let matcher = on_command::<SendCommand>("send")
//!     .handler(|cmd: CommandArgs<SendCommand>| async move {
//!         let user_id: &str = &cmd.target; // "12345"
//!         let image_ref: &str = &cmd.image; // "abc.jpg"
//!     });
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use clap::Parser;
use clap::error::ErrorKind;

use crate::extractor::FromContext;
use crate::handler::Handler;
use crate::matcher::Matcher;
use alloy_core::foundation::context::AlloyContext;
use alloy_core::foundation::error::ExtractError;
use alloy_core::foundation::message::RichTextSegment;

// Thread-local registry for resolving handles during clap's FromStr parsing.
thread_local! {
    static CURRENT_REGISTRY: RefCell<Option<HandleRegistry>> = RefCell::new(None);
}

// ============================================================================
// Command Extractor
// ============================================================================

/// A wrapper type for extracting parsed clap commands from context.
///
/// This extractor retrieves the command that was parsed during the matcher's
/// check phase. It requires that `on_command::<T>()` was used as the
/// matcher, which parses the command and stores it in the context.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_framework::CommandArgs;
///
/// async fn echo_handler(cmd: CommandArgs<BotCommand>) {
///     println!("Got command: {:?}", cmd.0);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CommandArgs<T: Parser>(pub T);

impl<T: Parser> CommandArgs<T> {
    /// Unwraps the command value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Parser> std::ops::Deref for CommandArgs<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Parser> std::ops::DerefMut for CommandArgs<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Parser + Clone + Send + Sync + 'static> FromContext for CommandArgs<T> {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        ctx.get_state::<ParsedCommand<T>>()
            .map(|parsed| CommandArgs(parsed.0.clone()))
            .ok_or_else(|| {
                ExtractError::custom(format!(
                    "CommandArgs<{}> not found in context. Make sure to use on_command::<T>() matcher.",
                    std::any::type_name::<T>()
                ))
            })
    }
}

/// Internal wrapper for storing parsed commands in context.
#[derive(Clone)]
struct ParsedCommand<T>(T);

// ============================================================================
// Rich Text Handles
// ============================================================================

/// Prefix used for image placeholder tokens in command argument strings.
const IMAGE_PLACEHOLDER_PREFIX: &str = "\x00IMG_";

/// Prefix used for at-mention placeholder tokens in command argument strings.
const AT_PLACEHOLDER_PREFIX: &str = "\x00AT_";

/// A shared registry mapping placeholder tokens to their original values.
///
/// Stored in [`AlloyContext`] so that `ImageSegment` and `AtSegment` can
/// look up their real data after clap parsing.
#[derive(Clone, Debug, Default)]
pub struct HandleRegistry {
    images: HashMap<String, String>,
    ats: HashMap<String, String>,
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
        let value = CURRENT_REGISTRY
            .with(|reg| reg.borrow().as_ref().and_then(|r| r.images.get(s).cloned()));
        match value {
            Some(v) => Ok(ImageSegment { value: v }),
            None => Err(format!("not a valid image segment: {}", s)),
        }
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
        let value =
            CURRENT_REGISTRY.with(|reg| reg.borrow().as_ref().and_then(|r| r.ats.get(s).cloned()));
        match value {
            Some(v) => Ok(AtSegment { value: v }),
            None => Err(format!("not a valid at segment: {}", s)),
        }
    }
}

// ============================================================================
// Shell Splitting (plain text)
// ============================================================================

/// Simple shell-like argument splitting for plain text.
///
/// Handles:
/// - Space-separated arguments
/// - Quoted strings (single and double quotes)
/// - Escape sequences within quotes
fn shell_split(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for ch in input.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_double_quote => {
                escape_next = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

// ============================================================================
// Rich Text Shell Splitting
// ============================================================================

/// Splits rich text segments into shell-like arguments.
///
/// This function processes a sequence of [`RichTextSegment`]s and produces
/// a list of string tokens suitable for clap parsing:
///
/// - **`Text`** segments are split using standard shell rules (whitespace
///   separation, quoted strings). A segment boundary always acts as a word
///   break, so text in separate segments is never concatenated.
/// - **`Image`** and **`At`** segments are replaced by unique placeholder
///   tokens (`\x00IMG_0`, `\x00AT_0`, etc.) that each become a single
///   argument.
///
/// Returns the argument list together with a [`HandleRegistry`] that maps
/// placeholders back to their original values.
fn rich_text_shell_split(segments: &[RichTextSegment]) -> (Vec<String>, HandleRegistry) {
    let mut args: Vec<String> = Vec::new();
    let mut registry = HandleRegistry::default();
    let mut img_counter: usize = 0;
    let mut at_counter: usize = 0;

    for seg in segments {
        match seg {
            RichTextSegment::Text(text) => {
                // Shell-split the text content; each resulting token becomes
                // its own argument. Segment boundaries act as whitespace.
                let sub_args = shell_split(text);
                args.extend(sub_args);
            }
            RichTextSegment::Image(reference) => {
                let placeholder = format!("{}{}", IMAGE_PLACEHOLDER_PREFIX, img_counter);
                img_counter += 1;
                registry
                    .images
                    .insert(placeholder.clone(), reference.clone());
                args.push(placeholder);
            }
            RichTextSegment::At(user_id) => {
                let placeholder = format!("{}{}", AT_PLACEHOLDER_PREFIX, at_counter);
                at_counter += 1;
                registry.ats.insert(placeholder.clone(), user_id.clone());
                args.push(placeholder);
            }
        }
    }

    (args, registry)
}

/// Creates a matcher that parses messages as clap commands.
///
/// This function creates a `Matcher` that:
/// 1. Only matches `Message` events
/// 2. Parses the plain text as a clap command
/// 3. Stores the parsed command in context for extraction via `CommandArgs<T>`
/// 4. Fails the check if parsing fails (optionally can reply with help)
///
/// # Type Parameters
///
/// - `T`: A type that implements `clap::Parser`
///
/// # Arguments
///
/// - `name`: The command name without "/" prefix (e.g., "calc" will match "/calc")
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
/// use alloy_framework::{on_command, CommandArgs};
///
/// #[derive(Parser, Clone)]
/// struct PingCommand {
///     /// Optional message to include
///     message: Option<String>,
/// }
///
/// let matcher = on_command::<PingCommand>("ping")
///     .reply_help(true)
///     .reply_error(true)
///     .handler(|cmd: CommandArgs<PingCommand>| async move {
///         println!("Ping! {:?}", cmd.message);
///     });
/// ```
pub fn on_command<T>(name: impl Into<String>) -> CommandMatcherBuilder<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    CommandMatcherBuilder::new(name.into())
}

/// Builder for command matchers.
pub struct CommandMatcherBuilder<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    /// Command name/prefix to match.
    name: String,
    /// Whether to reply with help message when user sends "-h" or "--help".
    reply_help: bool,
    /// Whether to reply with error message when parsing fails.
    reply_error: bool,
    _marker: PhantomData<T>,
}

impl<T> CommandMatcherBuilder<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    fn new(name: String) -> Self {
        Self {
            name,
            reply_help: true,
            reply_error: true,
            _marker: PhantomData,
        }
    }

    /// Enable automatic help message replies.
    ///
    /// When enabled, sends help message if user sends "<command> -h" or "<command> --help".
    pub fn reply_help(mut self, enabled: bool) -> Self {
        self.reply_help = enabled;
        self
    }

    /// Enable automatic error message replies.
    ///
    /// When enabled, sends error message when command parsing fails.
    pub fn reply_error(mut self, enabled: bool) -> Self {
        self.reply_error = enabled;
        self
    }

    /// Build into a `Matcher` with the given handler.
    pub fn handler<F, Params>(self, handler: F) -> Matcher
    where
        F: Handler<Params> + Send + Sync + 'static,
        Params: 'static,
    {
        self.build().handler(handler)
    }

    /// Build into a raw `Matcher` without handlers.
    ///
    /// You can add handlers using `.handler()` on the returned `Matcher`.
    pub fn build(self) -> Matcher {
        use alloy_core::foundation::event::EventType;

        let command_name = self.name.clone();
        let should_reply_help = self.reply_help;
        let should_reply_error = self.reply_error;

        Matcher::new().check(move |ctx| {
            // Must be a message event
            if ctx.event().event_type() != EventType::Message {
                return false;
            }

            // Use rich text to get segments for parsing
            let rich_text = ctx.event().get_rich_text();
            let (args, registry) = rich_text_shell_split(&rich_text);

            // Check if first argument matches "/{command_name}"
            let expected_cmd = format!("/{}", command_name);
            if args.is_empty() || args[0].to_lowercase() != expected_cmd.to_lowercase() {
                return false;
            }

            // Try to parse (set thread-local registry for handle resolution)
            CURRENT_REGISTRY.with(|reg| {
                *reg.borrow_mut() = Some(registry);
            });
            let result = T::try_parse_from(&args);
            CURRENT_REGISTRY.with(|reg| {
                *reg.borrow_mut() = None;
            });

            match result {
                Ok(cmd) => {
                    // Store parsed command in context
                    ctx.set_state(ParsedCommand(cmd));
                    true
                }
                Err(err) => {
                    // Check error kind to handle help/version requests
                    match err.kind() {
                        ErrorKind::DisplayHelp => {
                            if should_reply_help {
                                let help_text = err.to_string();
                                let bot = ctx.bot_arc();
                                let event = ctx.event().inner().clone();
                                tokio::spawn(async move {
                                    let _ = bot.send(event.as_ref(), &help_text).await;
                                });
                            }
                        }
                        _ => {
                            // Other parse errors
                            if should_reply_error {
                                let bot = ctx.bot_arc();
                                let event = ctx.event().inner().clone();
                                let error_msg = err.to_string();
                                tokio::spawn(async move {
                                    let _ = bot.send(event.as_ref(), &error_msg).await;
                                });
                            }
                        }
                    }
                    false
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_split_simple() {
        let args = shell_split("/echo hello world");
        assert_eq!(args, vec!["/echo", "hello", "world"]);
    }

    #[test]
    fn test_shell_split_quoted() {
        let args = shell_split(r#"/echo "hello world" test"#);
        assert_eq!(args, vec!["/echo", "hello world", "test"]);
    }

    #[test]
    fn test_shell_split_single_quoted() {
        let args = shell_split("/echo 'hello world' test");
        assert_eq!(args, vec!["/echo", "hello world", "test"]);
    }

    #[test]
    fn test_shell_split_mixed_quotes() {
        let args = shell_split(r#"/cmd "double's quote" 'single"s quote'"#);
        assert_eq!(args, vec!["/cmd", "double's quote", r#"single"s quote"#]);
    }

    #[test]
    fn test_shell_split_empty() {
        let args = shell_split("");
        assert!(args.is_empty());
    }

    #[test]
    fn test_shell_split_whitespace_only() {
        let args = shell_split("   \t  ");
        assert!(args.is_empty());
    }

    // ── Rich text splitting tests ──

    #[test]
    fn test_rich_text_split_text_only() {
        let segments = vec![RichTextSegment::Text("/echo hello world".into())];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args, vec!["/echo", "hello", "world"]);
        assert!(registry.images.is_empty());
        assert!(registry.ats.is_empty());
    }

    #[test]
    fn test_rich_text_split_with_image() {
        let segments = vec![
            RichTextSegment::Text("/send ".into()),
            RichTextSegment::Image("abc.jpg".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "/send");
        assert!(args[1].starts_with(IMAGE_PLACEHOLDER_PREFIX));
        assert_eq!(registry.images.get(&args[1]).unwrap(), "abc.jpg");
    }

    #[test]
    fn test_rich_text_split_with_at() {
        let segments = vec![
            RichTextSegment::Text("/kick ".into()),
            RichTextSegment::At("12345".into()),
            RichTextSegment::Text(" reason".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "/kick");
        assert!(args[1].starts_with(AT_PLACEHOLDER_PREFIX));
        assert_eq!(args[2], "reason");
        assert_eq!(registry.ats.get(&args[1]).unwrap(), "12345");
    }

    #[test]
    fn test_rich_text_split_mixed() {
        let segments = vec![
            RichTextSegment::Text("/cmd arg1 ".into()),
            RichTextSegment::At("user1".into()),
            RichTextSegment::Text(" ".into()),
            RichTextSegment::Image("pic.png".into()),
            RichTextSegment::Text(" arg2".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "/cmd");
        assert_eq!(args[1], "arg1");
        assert!(args[2].starts_with(AT_PLACEHOLDER_PREFIX));
        assert!(args[3].starts_with(IMAGE_PLACEHOLDER_PREFIX));
        assert_eq!(args[4], "arg2");
        assert_eq!(registry.images.len(), 1);
        assert_eq!(registry.ats.len(), 1);
    }

    #[test]
    fn test_rich_text_split_segment_boundary_breaks() {
        // Two text segments with no whitespace between them should still
        // act as separate tokens because segment boundaries break words.
        let segments = vec![
            RichTextSegment::Text("/echo".into()),
            RichTextSegment::Text("hello".into()),
        ];
        let (args, _) = rich_text_shell_split(&segments);
        assert_eq!(args, vec!["/echo", "hello"]);
    }

    #[test]
    fn test_handle_resolution_image() {
        let mut registry = HandleRegistry::default();
        let placeholder = format!("{}0", IMAGE_PLACEHOLDER_PREFIX);
        registry
            .images
            .insert(placeholder.clone(), "test.jpg".into());

        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = Some(registry);
        });
        let handle: ImageSegment = placeholder.parse().unwrap();
        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = None;
        });

        assert_eq!(&*handle, "test.jpg");
    }

    #[test]
    fn test_handle_resolution_at() {
        let mut registry = HandleRegistry::default();
        let placeholder = format!("{}0", AT_PLACEHOLDER_PREFIX);
        registry.ats.insert(placeholder.clone(), "99999".into());

        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = Some(registry);
        });
        let handle: AtSegment = placeholder.parse().unwrap();
        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = None;
        });

        assert_eq!(&*handle, "99999");
    }
}
