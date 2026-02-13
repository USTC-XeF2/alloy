//! Alloy Framework Echo Bot Example
//!
//! A simple bot demonstration using the Alloy framework, featuring message logging,
//! basic commands, and group-specific handling.
//!
//! # Running the Example
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

// --- Command Definitions ---

// An empty command with no arguments.
#[derive(Parser, Debug, Clone)]
pub struct EmptyCommand;

/// Arguments for the `/echo` command.
#[derive(Parser, Debug, Clone)]
struct EchoCommand {
    /// The text to be echoed back.
    text: Vec<String>,
}

/// Arguments for the `/calc` command, supporting various mathematical operations.
///
/// Example usage:
/// - `/calc add 5 10`
/// - `/calc multiply 3 4`
#[derive(Parser, Debug, Clone)]
struct CalcCommand {
    #[command(subcommand)]
    operation: CalcOperation,
}

#[derive(Parser, Debug, Clone)]
enum CalcOperation {
    /// Adds two integers.
    Add { a: i32, b: i32 },
    /// Multiplies two integers.
    Multiply { a: i32, b: i32 },
}

// --- Event Handlers ---

/// A simple logging handler that records every incoming message.
///
/// This handler demonstrates how to use `EventContext<MessageEvent>` to access
/// common message information like the sender's nickname and message content.
async fn logging_handler(event: EventContext<MessageEvent>) {
    let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");

    info!(
        "[Message] {} ({}): {}",
        nickname,
        event.user_id,
        event.get_plain_text()
    );
}

/// Handles the `/echo` command by sending the provided text back to the source.
async fn echo_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: CommandArgs<EchoCommand>,
) {
    let content = cmd.text.join(" ");
    if !content.is_empty() {
        if let Err(e) = bot.send(event.as_event(), &content).await {
            error!("Failed to send echo reply: {:?}", e);
        }
    }
}

/// Handles the `/ping` command with a "Pong!" response.
async fn ping_handler(event: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    if let Err(e) = bot.send(event.as_event(), "Pong! 🏓").await {
        error!("Failed to send ping reply: {:?}", e);
    }
}

/// Displays information about the current message and its sender.
async fn info_handler(event: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
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

/// A handler that only responds to group messages.
///
/// By using `EventContext<GroupMessageEvent>`, this handler will only be
/// triggered when the event is a group message. Alloy's dispatcher handles
/// this filtering automatically.
async fn group_only_handler(event: EventContext<GroupMessageEvent>, bot: Arc<OneBotBot>) {
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

/// A subcommand-based handler for mathematical calculations.
async fn calc_handler(
    event: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: CommandArgs<CalcCommand>,
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

// --- Main Application ---

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the Alloy runtime.
    // By default, it loads configuration from `alloy.yaml` in the current directory.
    // Configuration can also be overridden via environment variables using the prefixed format.
    let runtime = AlloyRuntime::new();

    // Register the OneBot adapter.
    // The adapter will automatically use the connection settings defined in the configuration.
    runtime.register_adapter::<OneBotAdapter>().await?;

    // Register matchers to define how the bot should respond to events.
    runtime
        .register_matchers(vec![
            // A non-blocking matcher that logs every message received.
            on_message().block(false).handler(logging_handler),
            // Command matchers use `on_command` to bridge message text and structured data.
            // They automatically handle prefix stripping and parsing.
            on_command::<EchoCommand>("echo").handler(echo_handler),
            on_command::<EmptyCommand>("ping").handler(ping_handler),
            on_command::<EmptyCommand>("info").handler(info_handler),
            on_command::<EmptyCommand>("group").handler(group_only_handler),
            on_command::<CalcCommand>("calc").handler(calc_handler),
        ])
        .await;

    // Start the bot and wait for it to finish.
    runtime.run().await?;

    Ok(())
}
