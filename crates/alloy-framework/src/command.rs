//! Command parsing system using clap.
//!
//! This module provides a clap-based command parsing system that integrates
//! with the Alloy matcher system. It supports:
//!
//! - Automatic command parsing from message plain text
//! - Type-safe command extraction via `Command<T>` extractor
//! - Help message generation on parse errors
//!
//! # Example
//!
//! ```rust,ignore
//! use clap::{Parser, Subcommand};
//! use alloy_framework::{on_command, Command};
//!
//! #[derive(Parser, Clone)]
//! #[command(name = "bot", about = "Bot commands")]
//! struct BotCommand {
//!     #[command(subcommand)]
//!     cmd: BotSubcommand,
//! }
//!
//! #[derive(Subcommand, Clone)]
//! enum BotSubcommand {
//!     /// Echo a message back
//!     Echo { message: String },
//!     /// Show help
//!     Help,
//! }
//!
//! // Handle all subcommands in one handler
//! let matcher = on_command::<BotCommand>("bot")
//!     .handler(|cmd: Command<BotCommand>| async move {
//!         match cmd.cmd {
//!             BotSubcommand::Echo { message } => println!("Echo: {}", message),
//!             BotSubcommand::Help => println!("Help!"),
//!         }
//!     });
//! ```

use std::marker::PhantomData;

use clap::Parser;

use crate::extractor::FromContext;
use crate::handler::Handler;
use crate::matcher::Matcher;
use alloy_core::foundation::context::AlloyContext;
use alloy_core::foundation::error::ExtractError;

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
/// use alloy_framework::Command;
///
/// async fn echo_handler(cmd: Command<BotCommand>) {
///     println!("Got command: {:?}", cmd.0);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Command<T>(pub T);

impl<T> Command<T> {
    /// Unwraps the command value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for Command<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Command<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Clone + Send + Sync + 'static> FromContext for Command<T> {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        ctx.get_state::<ParsedCommand<T>>()
            .map(|parsed| Command(parsed.0.clone()))
            .ok_or_else(|| {
                ExtractError::custom(format!(
                    "Command<{}> not found in context. Make sure to use on_command::<T>() matcher.",
                    std::any::type_name::<T>()
                ))
            })
    }
}

/// Internal wrapper for storing parsed commands in context.
#[derive(Clone)]
struct ParsedCommand<T>(T);

// ============================================================================
// Command Parsing Utilities
// ============================================================================

/// Parse a command from plain text.
///
/// This function:
/// 1. Splits the text into shell-like arguments
/// 2. Uses clap to parse them into the target type
///
/// Returns `Ok(command)` on success, or `Err(error_message)` on failure.
pub fn parse_command<T: Parser>(text: &str) -> Result<T, String> {
    // Split text into arguments (simple shell-like splitting)
    let args = shell_split(text);

    // Try to parse using clap
    T::try_parse_from(args).map_err(|e| e.to_string())
}

/// Parse command and return both the command and any remaining text.
pub fn parse_command_with_remainder<T: Parser>(text: &str) -> Result<(T, Vec<String>), String> {
    let args = shell_split(text);

    // Use clap's FromArgMatches for more control
    let mut cmd = T::command();
    let matches = cmd
        .try_get_matches_from_mut(args)
        .map_err(|e| e.to_string())?;

    let command = T::from_arg_matches(&matches).map_err(|e| e.to_string())?;

    // Get any remaining arguments (if the command allows them)
    // For now, we don't capture remainder
    Ok((command, vec![]))
}

/// Get the help message for a command.
pub fn get_help<T: Parser>() -> String {
    T::command().render_help().to_string()
}

/// Get the short help message for a command.
pub fn get_short_help<T: Parser>() -> String {
    T::command().render_help().to_string()
}

/// Simple shell-like argument splitting.
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
// on_command - Simple Matcher Builder
// ============================================================================

/// Creates a matcher that parses messages as clap commands.
///
/// This function creates a `Matcher` that:
/// 1. Only matches `Message` events
/// 2. Parses the plain text as a clap command
/// 3. Stores the parsed command in context for extraction via `Command<T>`
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
/// use alloy_framework::{on_command, Command};
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
///     .handler(|cmd: Command<PingCommand>| async move {
///         println!("Ping! {:?}", cmd.message);
///     });
/// ```
pub fn on_command<T>(name: impl Into<String>) -> CommandMatcherBuilder<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    let name = name.into();
    // Auto-prepend "/" if not present
    let full_name = if name.starts_with('/') {
        name
    } else {
        format!("/{name}")
    };
    CommandMatcherBuilder::new(full_name)
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
            reply_help: false,
            reply_error: false,
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

        Matcher::new()
            .name(format!("command:{}", command_name.trim_start_matches('/')))
            .check(move |ctx| {
                // Must be a message event
                if ctx.event().event_type() != EventType::Message {
                    return false;
                }

                let text = ctx.event().plain_text();
                let text = text.trim();

                // Check if message starts with command name
                let starts_with_command = text
                    .to_lowercase()
                    .starts_with(&command_name.to_lowercase());
                if !starts_with_command {
                    return false;
                }

                // Check for help flag
                if should_reply_help && (text.contains("-h") || text.contains("--help")) {
                    let help_text = get_help::<T>();
                    let bot = ctx.bot_arc();
                    let event = ctx.event().inner().clone();
                    tokio::spawn(async move {
                        let _ = bot.send(event.as_ref(), &help_text).await;
                    });
                    return false;
                }

                // Try to parse
                match parse_command::<T>(text) {
                    Ok(cmd) => {
                        // Store parsed command in context
                        ctx.set_state(ParsedCommand(cmd));
                        true
                    }
                    Err(err) => {
                        // Send error message if enabled
                        if should_reply_error {
                            let bot = ctx.bot_arc();
                            let event = ctx.event().inner().clone();
                            let error_msg = format!("❌ Command error:\n{err}");
                            tokio::spawn(async move {
                                let _ = bot.send(event.as_ref(), &error_msg).await;
                            });
                        }
                        false
                    }
                }
            })
    }
}

// ============================================================================
// Tests
// ============================================================================

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

    // Tests below require clap derive feature
    #[cfg(feature = "command-test")]
    mod derive_tests {
        use super::*;
        use clap::Parser;

        #[derive(Parser, Debug, Clone, PartialEq)]
        #[command(name = "/test")]
        struct TestCommand {
            /// A required argument
            arg: String,
            /// An optional flag
            #[arg(short, long)]
            flag: bool,
        }

        #[test]
        fn test_parse_command_simple() {
            let cmd: TestCommand = parse_command("/test hello").unwrap();
            assert_eq!(cmd.arg, "hello");
            assert!(!cmd.flag);
        }

        #[test]
        fn test_parse_command_with_flag() {
            let cmd: TestCommand = parse_command("/test hello -f").unwrap();
            assert_eq!(cmd.arg, "hello");
            assert!(cmd.flag);
        }

        #[test]
        fn test_parse_command_long_flag() {
            let cmd: TestCommand = parse_command("/test hello --flag").unwrap();
            assert_eq!(cmd.arg, "hello");
            assert!(cmd.flag);
        }

        #[test]
        fn test_parse_command_failure() {
            let result: Result<TestCommand, _> = parse_command("/test");
            assert!(result.is_err());
        }

        #[derive(Parser, Debug, Clone)]
        #[command(name = "/bot")]
        struct BotCommand {
            #[command(subcommand)]
            cmd: BotSubcommand,
        }

        #[derive(clap::Subcommand, Debug, Clone)]
        enum BotSubcommand {
            Echo { message: String },
            Status,
        }

        #[test]
        fn test_parse_subcommand() {
            let cmd: BotCommand = parse_command("/bot echo hello").unwrap();
            match cmd.cmd {
                BotSubcommand::Echo { message } => assert_eq!(message, "hello"),
                _ => panic!("Wrong subcommand"),
            }
        }

        #[test]
        fn test_parse_subcommand_status() {
            let cmd: BotCommand = parse_command("/bot status").unwrap();
            assert!(matches!(cmd.cmd, BotSubcommand::Status));
        }
    }
}
