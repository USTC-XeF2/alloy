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
use std::sync::Arc;

use alloy::builtin_plugins::storage::{STORAGE_PLUGIN, StorageService};
use alloy::prelude::*;
use alloy_adapter_onebot::{GroupMessageEvent, MessageEvent, OneBotAdapter, OneBotBot};
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
async fn logging_handler(event: EventContext<MessageEvent>) {
    let nickname = event.sender.nickname.as_deref().unwrap_or("Unknown");

    info!(
        "[Message] {} ({}): {}",
        nickname,
        event.user_id,
        event.get_plain_text()
    );
}

/// Echoes the provided text back to the sender.
async fn echo_handler(cmd: CommandArgs<EchoCommand>) -> Option<String> {
    Some(cmd.text.join(" ")).filter(|s| !s.is_empty())
}

/// Displays group information or member details (if `--user` is provided).
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

/// Records one sign-in per user per calendar day (UTC) to `signin.json`.
async fn signin_handler(
    event: EventContext<MessageEvent>,
    storage: ServiceRef<dyn StorageService>,
) -> Result<RichText> {
    let path = storage.data_dir().join("signin.json");

    // Load existing records (user_id → last-signin-date).
    let mut records: HashMap<String, String> = if path.exists() {
        let text = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let user_id = event.user_id.to_string();

    let format = format_description!("[year]-[month]-[day]");
    let Some(today) = OffsetDateTime::now_local()
        .ok()
        .and_then(|dt| dt.format(format).ok())
    else {
        return Ok(RichText::msg(
            "获取当前日期失败，请稍后再试！",
            event.get_user_id(),
        ));
    };

    if records.get(&user_id).is_some_and(|d| d == &today) {
        return Ok(RichText::msg("你今天已经签到过了！", event.get_user_id()));
    }

    records.insert(user_id, today);
    let json = serde_json::to_string_pretty(&records)?;
    tokio::fs::write(&path, json).await?;

    Ok(RichText::msg("签到成功！", event.get_user_id()))
}

/// The echo bot plugin with command handlers for echo, info, and signin.
static ECHO_PLUGIN: PluginDescriptor = define_plugin! {
    name: "echo_bot",
    depends_on: [StorageService],
    handlers: [
        on_message().handler(logging_handler),
        on_command::<EchoCommand>("echo").handler(echo_handler),
        on_command::<InfoCommand>("info").handler(info_handler),
        on_command::<SigninCommand>("signin").handler(signin_handler),
    ],
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize runtime and register adapter
    let runtime = AlloyRuntime::new();
    runtime.register_adapter::<OneBotAdapter>()?;

    // Load plugins
    runtime.register_plugin(&STORAGE_PLUGIN);
    runtime.register_plugin(&ECHO_PLUGIN);

    runtime.run().await;
    Ok(())
}
