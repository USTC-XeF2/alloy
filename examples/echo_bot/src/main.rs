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

use std::sync::Arc;

use alloy::prelude::*;
use alloy_adapter_onebot::{GroupMessageEvent, MessageEvent, OneBotAdapter, OneBotBot};
use anyhow::Result;
use clap::Parser;
use tracing::info;

// --- Command Definitions ---

/// Arguments for the `/echo` command.
#[derive(Parser, Debug, Clone)]
struct EchoCommand {
    /// The text to be echoed back.
    text: Vec<String>,
}

/// Arguments for the `/info` command.
#[derive(Parser, Debug, Clone)]
struct InfoCommand {
    /// Optional user to query. Uses @mention syntax.
    #[arg(short, long)]
    user: Option<AtSegment>,
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
async fn echo_handler(cmd: CommandArgs<EchoCommand>) -> Result<String> {
    Ok(cmd.text.join(" ")) // Empty string results in no message sent
}

/// Displays information about the current group and optionally queries a specific member.
///
/// This handler only works in group channels. If no user is specified with `--user`,
/// it displays group information. If a user is specified (using @mention syntax),
/// it queries member information via the OneBot API.
async fn info_handler(
    event: EventContext<GroupMessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: CommandArgs<InfoCommand>,
) -> Result<String> {
    if let Some(user_id) = &cmd.user {
        // Parse user ID - user input error, return as message
        let Ok(parsed_id) = user_id.parse::<i64>() else {
            return Ok(format!("Invalid User ID: {user_id}"));
        };

        // Query member information - API error, let framework handle it
        let member = bot
            .get_group_member_info(event.group_id, parsed_id, false)
            .await?;

        Ok(format!(
            "Member Info\n\
             • Name: {}\n\
             • User ID: {}\n\
             • Title: {}\n\
             • Joined: {}",
            member.nickname, member.user_id, member.title, member.join_time
        ))
    } else {
        // Display group information
        let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");

        Ok(format!(
            "Group Info\n\
             • Group ID: {}\n\
             • From: {} ({})\n\
             • Message ID: {}\n\
             • Type: {}",
            event.group_id, nickname, event.user_id, event.message_id, event.message_type
        ))
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
            on_command::<InfoCommand>("info").handler(info_handler),
        ])
        .await;

    // Start the bot and wait for it to finish.
    runtime.run().await;

    Ok(())
}
