//! Alloy Framework Echo Bot Example
//!
//! Demonstrates the plugin system, command parsing, and service interaction.
//! Includes three commands: `/echo`, `/info`, and `/signin`.
//!
//! # Running
//!
//! ```bash
//! cargo run --package echo-bot
//! ```

use std::collections::HashMap;

use alloy::prelude::*;
use alloy_adapter_onebot::{GroupMessageEvent, MessageEvent, OneBotAdapter, OneBotBot};
use alloy_plugin_storage::{Data, STORAGE_PLUGIN, StorageDir, StorageService};
use anyhow::Result;
use clap::Parser;
use time::OffsetDateTime;
use time::macros::format_description;
use tracing::info;

/// Echo back the provided text.
#[derive(Parser, Debug, Clone)]
struct EchoCommand {
    /// The text to be echoed back.
    text: Vec<String>,
}

/// Display group or member information.
#[derive(Parser, Debug, Clone)]
struct InfoCommand {
    /// Optional user to query. Uses @mention syntax.
    #[arg(short, long)]
    user: Option<AtSegment>,
}

/// Sign in once per calendar day (UTC). Records persisted to `signin.json`.
#[derive(Parser, Debug, Clone)]
struct SigninCommand {}

/// Logs every incoming message.
async fn logging_handler(event: Event<MessageEvent>) {
    info!(
        "[Message] {} ({}): {}",
        event.sender().nickname,
        event.sender().user_id,
        event.plain_text()
    );
}

/// Echoes the provided text back to the sender.
async fn echo_handler(cmd: CommandArgs<EchoCommand>) -> Option<String> {
    Some(cmd.text.join(" ")).filter(|s| !s.is_empty())
}

/// Displays group information or member details (if `--user` is provided).
async fn info_handler(
    event: Event<GroupMessageEvent>,
    bot: Bot<OneBotBot>,
    cmd: CommandArgs<InfoCommand>,
) -> Result<String> {
    if let Some(user) = &cmd.user {
        let Some(user_id) = user.as_ref() else {
            return Ok("Invalid User ID: @all is not supported.".to_string());
        };

        // Parse user ID - user input error, return as message
        let Ok(user_id) = user_id.parse::<i64>() else {
            return Ok(format!("Invalid User ID: {user_id}"));
        };

        // Query member information - API error, let framework handle it
        let member = bot.get_group_member_info(event.group_id, user_id).await?;

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
        Ok(format!(
            "Group Info\n\
             • Group ID: {}\n\
             • From: {} ({})\n\
             • Message ID: {}",
            event.group_id, event.sender.nickname, event.user_id, event.message_id
        ))
    }
}

/// Records one sign-in per user per calendar day (UTC) to `signin.json`.
async fn signin_handler(
    event: Event<MessageEvent>,
    data_dir: StorageDir<Data>,
) -> Result<RichText> {
    let path = data_dir.join("signin.json");

    // Load existing records.
    let mut records: HashMap<String, String> = if path.exists() {
        let text = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let user_id = event.sender().user_id.to_string();
    let msg = RichText::new().at(user_id.clone());

    let format = format_description!("[year]-[month]-[day]");
    let Some(today) = OffsetDateTime::now_local()
        .ok()
        .and_then(|dt| dt.format(format).ok())
    else {
        return Ok(msg.text("Failed to get current date."));
    };

    if records.get(&user_id).is_some_and(|d| d == &today) {
        return Ok(msg.text("You have already signed in today!"));
    }

    records.insert(user_id, today);
    let json = serde_json::to_string_pretty(&records)?;
    tokio::fs::write(&path, json).await?;

    Ok(msg.text("Sign-in successful!"))
}

async fn on_load(ctx: PluginLoadContext) -> Result<()> {
    // Register commands with the framework, associating them with their respective handlers.
    ctx.register_commands(async || {
        CommandMap::new()
            .insert::<EchoCommand>("echo")
            .insert::<InfoCommand>("info")
            .insert::<SigninCommand>("signin")
    });
    Ok(())
}

define_plugin! {
    /// The echo bot plugin with command handlers for echo, info, and signin.
    name: "echo_bot",
    depends_on: [StorageService],
    on_load: on_load,
    handlers: [
        on_message().handler(logging_handler),
        on_command::<EchoCommand>("echo").handler(echo_handler),
        on_command::<InfoCommand>("info").handler(info_handler),
        on_command::<SigninCommand>("signin").handler(signin_handler),
        help_command(),
    ],
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize runtime and register adapter
    let runtime = AlloyRuntime::new();
    runtime.register_adapter::<OneBotAdapter>()?;

    // Load plugins
    runtime.register_plugin(&STORAGE_PLUGIN);
    runtime.register_plugin(&ECHO_BOT_PLUGIN);

    runtime.run().await;
    Ok(())
}
