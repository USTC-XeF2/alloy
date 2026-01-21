//! Command parsing system using clap.
//!
//! This module provides a clap-based command parsing system that integrates
//! with the Alloy matcher system. It supports:
//!
//! - Automatic command parsing from message plain text
//! - Type-safe command extraction via `Command<T>` extractor
//! - Subcommand routing to different handlers
//! - Help message generation on parse errors
//!
//! # Example
//!
//! ```rust,ignore
//! use clap::{Parser, Subcommand};
//! use alloy_framework::{on_command_struct, Command};
//!
//! #[derive(Parser, Clone)]
//! #[command(name = "/bot", about = "Bot commands")]
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
//! // Method 1: Handle all subcommands in one handler
//! let matcher = on_command_struct::<BotCommand>()
//!     .handler(|cmd: Command<BotCommand>| async move {
//!         match cmd.cmd {
//!             BotSubcommand::Echo { message } => println!("Echo: {}", message),
//!             BotSubcommand::Help => println!("Help!"),
//!         }
//!     });
//!
//! // Method 2: Route subcommands to different handlers
//! let matcher = CommandRouter::<BotCommand>::new()
//!     .route("echo", echo_handler)
//!     .route("help", help_handler)
//!     .build();
//! ```

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use clap::Parser;

use crate::extractor::FromContext;
use crate::handler::{BoxedHandler, Handler, into_handler};
use crate::matcher::Matcher;
use alloy_core::foundation::context::AlloyContext;
use alloy_core::foundation::error::ExtractError;

// ============================================================================
// Command Extractor
// ============================================================================

/// A wrapper type for extracting parsed clap commands from context.
///
/// This extractor retrieves the command that was parsed during the matcher's
/// check phase. It requires that `on_command_struct::<T>()` was used as the
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
            .ok_or_else(|| ExtractError::custom(format!(
                "Command<{}> not found in context. Make sure to use on_command_struct::<T>() matcher.",
                std::any::type_name::<T>()
            )))
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
// on_command_struct - Simple Matcher Builder
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
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
/// use alloy_framework::{on_command_struct, Command};
///
/// #[derive(Parser, Clone)]
/// struct PingCommand {
///     /// Optional message to include
///     message: Option<String>,
/// }
///
/// let matcher = on_command_struct::<PingCommand>("/ping")
///     .reply_help(true)
///     .reply_error(true)
///     .handler(|cmd: Command<PingCommand>| async move {
///         println!("Ping! {:?}", cmd.message);
///     });
/// ```
pub fn on_command_struct<T>(name: impl Into<String>) -> CommandMatcherBuilder<T>
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

    /// Create a router for dispatching subcommands to different handlers.
    pub fn router(self) -> CommandRouter<T> {
        CommandRouter::new(self.name, self.reply_help, self.reply_error)
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
            .name(format!("command_struct:{command_name}"))
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
// CommandRouter - Subcommand Routing
// ============================================================================

/// A router that dispatches subcommands to different handlers.
///
/// This is useful when you have an enum of subcommands and want each
/// subcommand to be handled by a different function.
///
/// # Example
///
/// ```rust,ignore
/// use clap::{Parser, Subcommand};
/// use alloy_framework::CommandRouter;
///
/// #[derive(Parser, Clone)]
/// struct BotCommand {
///     #[command(subcommand)]
///     cmd: BotSubcommand,
/// }
///
/// #[derive(Subcommand, Clone)]
/// enum BotSubcommand {
///     Echo { message: String },
///     Help,
/// }
///
/// // Route each subcommand to a different handler
/// let matcher = CommandRouter::<BotCommand>::new()
///     .route("echo", |cmd: Command<BotCommand>| async move {
///         // Handle echo
///     })
///     .route("help", |cmd: Command<BotCommand>| async move {
///         // Handle help
///     })
///     .build();
/// ```
///
/// # Note
///
/// The handler still receives `Command<BotCommand>` (the full command),
/// not just the subcommand variant. You need to match on the subcommand
/// in your handler.
pub struct CommandRouter<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    /// Command name/prefix.
    name: String,
    /// Whether to reply with help message.
    reply_help: bool,
    /// Whether to reply with error message.
    reply_error: bool,
    routes: HashMap<String, BoxedHandler>,
    /// Fallback handler for unrouted subcommands.
    fallback: Option<BoxedHandler>,
    _marker: PhantomData<T>,
}

impl<T> CommandRouter<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    /// Create a new command router from builder settings.
    fn new(name: String, reply_help: bool, reply_error: bool) -> Self {
        Self {
            name,
            reply_help,
            reply_error,
            routes: HashMap::new(),
            fallback: None,
            _marker: PhantomData,
        }
    }

    /// Route a subcommand to a handler.
    ///
    /// The `subcommand` should match the kebab-case name that clap generates
    /// (e.g., "my-command" for a variant named `MyCommand`).
    pub fn route<F, Params>(mut self, subcommand: impl Into<String>, handler: F) -> Self
    where
        F: Handler<Params> + Send + Sync + 'static,
        Params: 'static,
    {
        self.routes.insert(subcommand.into(), into_handler(handler));
        self
    }

    /// Set a fallback handler for unrouted subcommands.
    pub fn fallback<F, Params>(mut self, handler: F) -> Self
    where
        F: Handler<Params> + Send + Sync + 'static,
        Params: 'static,
    {
        self.fallback = Some(into_handler(handler));
        self
    }

    /// Build into a `Matcher`.
    pub fn build(self) -> Matcher {
        use alloy_core::foundation::event::EventType;

        let command_name = self.name.clone();
        let should_reply_help = self.reply_help;
        let should_reply_error = self.reply_error;
        let routes = Arc::new(self.routes);
        let fallback = self.fallback;

        let routes_for_check = Arc::clone(&routes);
        let has_fallback = fallback.is_some();

        // Create a matcher that:
        // 1. Parses the command
        // 2. Identifies which subcommand was used
        // 3. Stores both the command and the subcommand name

        let mut matcher = Matcher::new()
            .name(format!("command_router:{command_name}"))
            .check(move |ctx| {
                if ctx.event().event_type() != EventType::Message {
                    return false;
                }

                let text = ctx.event().plain_text();
                let text = text.trim();

                // Check prefix
                if !text
                    .to_lowercase()
                    .starts_with(&command_name.to_lowercase())
                {
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
                        // Identify subcommand from the text
                        let args = shell_split(text);
                        let subcommand_name = args.get(1).cloned().unwrap_or_default();

                        // Check if we have a route for this subcommand
                        let has_route =
                            routes_for_check.contains_key(&subcommand_name) || has_fallback;

                        if has_route {
                            ctx.set_state(ParsedCommand(cmd));
                            ctx.set_state(SubcommandName(subcommand_name));
                            true
                        } else {
                            false
                        }
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
            });

        // Add a routing handler
        let routes_for_handler = routes;
        let fallback_for_handler = fallback;

        matcher = matcher.handler_boxed(Arc::new(RoutingHandler {
            routes: routes_for_handler,
            fallback: fallback_for_handler,
        }));

        matcher
    }
}

/// Stores the subcommand name for routing.
#[derive(Clone)]
struct SubcommandName(String);

/// Internal handler that routes to subcommand handlers.
struct RoutingHandler {
    routes: Arc<HashMap<String, BoxedHandler>>,
    fallback: Option<BoxedHandler>,
}

impl crate::handler::ErasedHandler for RoutingHandler {
    fn call(&self, ctx: Arc<AlloyContext>) -> crate::handler::BoxFuture<'static, ()> {
        let routes = Arc::clone(&self.routes);
        let fallback = self.fallback.clone();

        Box::pin(async move {
            // Get the subcommand name
            let subcommand = ctx
                .get_state::<SubcommandName>()
                .map(|s| s.0.clone())
                .unwrap_or_default();

            // Find the handler
            if let Some(handler) = routes.get(&subcommand) {
                handler.call(ctx).await;
            } else if let Some(handler) = fallback {
                handler.call(ctx).await;
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
