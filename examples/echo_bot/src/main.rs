//! Echo Bot Example
//!
//! A simple demonstration of the Alloy framework using parent-in-child events.
//!
//! # Parent-in-Child Event Design
//!
//! Events use `Deref` chains so child events transparently access parent fields:
//!
//! ```text
//! PrivateMessageEvent  ──Deref──▶  MessageEvent  ──Deref──▶  OneBotEvent
//!   sub_type                        user_id, message, …       time, self_id
//! ```
//!
//! # Usage
//!
//! ```bash
//! cargo run --package echo-bot
//! ```

use alloy::prelude::*;
use alloy_adapter_onebot::{GroupMessageEvent, MessageEvent, OneBotAdapter, OneBotBot};
use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::{error, info};

// ============================================================================
// Command Definitions (using clap)
// ============================================================================

/// Echo command - echoes back the given text
#[derive(Parser, Debug, Clone)]
#[command(name = "/echo")]
struct EchoCommand {
    /// Text to echo back
    text: Vec<String>,
}

/// Ping command - responds with Pong!
#[derive(Parser, Debug, Clone)]
#[command(name = "/ping")]
struct PingCommand;

/// Help command - shows help message
#[derive(Parser, Debug, Clone)]
#[command(name = "/help")]
struct HelpCommand;

/// Info command - shows message info
#[derive(Parser, Debug, Clone)]
#[command(name = "/info")]
struct InfoCommand;

/// Group-only command - only works in groups
#[derive(Parser, Debug, Clone)]
#[command(name = "/group")]
struct GroupCommand;

/// Calculator command with subcommands
///
/// Usage:
/// - /calc add 5 10
/// - /calc multiply 3 4
#[derive(Parser, Debug, Clone)]
#[command(name = "/calc")]
struct CalcCommand {
    #[command(subcommand)]
    operation: CalcOperation,
}

#[derive(Parser, Debug, Clone)]
enum CalcOperation {
    /// Add two numbers
    Add { a: i32, b: i32 },
    /// Multiply two numbers
    Multiply { a: i32, b: i32 },
}

// ============================================================================
// Handler Functions (Axum-style - no macro needed!)
// ============================================================================

/// Logging handler - logs all messages.
///
/// This handler runs for every message event.
/// Uses `MessageEvent` which provides common fields for all message types.
async fn logging_handler(event: EventContext<MessageEvent>) {
    let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");

    info!(
        "[Message] {} ({}): {}",
        nickname,
        event.user_id,
        event.plain_text()
    );
}

/// Echo command handler - sends back the message!
async fn echo_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: Command<EchoCommand>,
) {
    let content = cmd.text.join(" ");
    if !content.is_empty() {
        if let Err(e) = bot.send(event.as_event(), &content).await {
            error!("Failed to send echo reply: {:?}", e);
        }
    }
}

/// Ping command handler - responds with Pong!
async fn ping_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    _cmd: Command<PingCommand>,
) {
    if let Err(e) = bot.send(event.as_event(), "Pong! 🏓").await {
        error!("Failed to send ping reply: {:?}", e);
    }
}

/// Help command handler - sends help message.
async fn help_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    _cmd: Command<HelpCommand>,
) {
    let help_text = r"╭─────────────────────────────╮
│     Echo Bot - Commands     │
├─────────────────────────────┤
│ /echo <text> - Echo text    │
│ /ping        - Pong!        │
│ /help        - This help    │
│ /info        - Message info │
│ /group       - Group only   │
│ /calc add <a> <b>           │
│ /calc multiply <a> <b>      │
╰─────────────────────────────╯";

    if let Err(e) = bot.send(event.as_event(), help_text).await {
        error!("Failed to send help message: {:?}", e);
    }
}

/// Info command handler - sends message info.
async fn info_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    _cmd: Command<InfoCommand>,
) {
    let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");

    let info_text = format!(
        "📋 Message Info\n\
        • From: {} ({})\n\
        • Message ID: {}\n\
        • Type: {}",
        nickname, event.user_id, event.message_id, event.message_type
    );

    if let Err(e) = bot.send(event.as_event(), &info_text).await {
        error!("Failed to send info message: {:?}", e);
    }
}

/// Group-only command handler - responds only in groups.
/// Uses `GroupMessageEvent` directly — auto-extracts only for group messages.
async fn group_only_handler(
    event: EventContext<GroupMessageEvent>,
    bot: Arc<OneBotBot>,
    _cmd: Command<GroupCommand>,
) {
    let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");
    let response = format!(
        "✅ This is a group-only command!\n\
            • Group ID: {}\n\
            • User: {} ({})",
        event.group_id, nickname, event.user_id
    );

    if let Err(e) = bot.send(event.as_event(), &response).await {
        error!("Failed to send group-only response: {:?}", e);
    }
}

/// Calculator command handler - handles add and multiply operations
async fn calc_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: Command<CalcCommand>,
) {
    let response = match &cmd.operation {
        CalcOperation::Add { a, b } => {
            let result = a + b;
            format!("➕ {a} + {b} = {result}")
        }
        CalcOperation::Multiply { a, b } => {
            let result = a * b;
            format!("✖️ {a} × {b} = {result}")
        }
    };

    if let Err(e) = bot.send(event.as_event(), &response).await {
        error!("Failed to send calc result: {:?}", e);
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Create runtime - automatically loads config from alloy.yaml
    // Config can be overridden via environment variables:
    // - ALLOY_LOGGING__LEVEL=debug
    // - ALLOY_ADAPTERS__ONEBOT__CONNECTIONS__0__URL=ws://...
    let runtime = AlloyRuntime::new();

    // Register the OneBot adapter - configuration is automatically loaded from alloy.yaml
    // The adapter name "onebot" is defined in OneBotAdapter::adapter_name()
    runtime.register_adapter::<OneBotAdapter>().await?;

    // ========================================================================
    // Register Matchers
    // ========================================================================

    runtime
        .register_matchers(vec![
            // Logging - runs for all message events, does NOT block
            on_message()
                .block(false) // Don't block - let other matchers also process
                .handler(logging_handler),
            // Command handlers - use on_command::<T>() with clap parsing
            on_command::<EchoCommand>("echo").handler(echo_handler),
            on_command::<PingCommand>("ping").handler(ping_handler),
            on_command::<HelpCommand>("help").handler(help_handler),
            on_command::<InfoCommand>("info").handler(info_handler),
            on_command::<GroupCommand>("group").handler(group_only_handler),
            // Calculator command with subcommands
            // Automatic help (-h) and error messages
            // Example: /calc add 5 10
            //          /calc multiply 3 4
            //          /calc -h (shows help)
            on_command::<CalcCommand>("calc")
                .reply_help(true)
                .reply_error(true)
                .handler(calc_handler),
        ])
        .await;

    // Run the runtime
    runtime.run().await?;

    Ok(())
}
