//! Echo Bot Example
//!
//! A simple demonstration of the Alloy framework using the new Matcher system
//! with Axum-style handler functions.
//!
//! # Matcher System
//!
//! Matchers group handlers with a common check rule:
//! - When the check passes, all handlers in the matcher are executed
//! - A blocking matcher stops further matchers from processing
//!
//! # Event Extraction
//!
//! Handlers use `EventContext<T>` to extract events at any level:
//!
//! ```text
//! OneBotEvent { time, self_id, inner: OneBotEventKind }
//! └── OneBotEventKind::Message(MessageEvent)
//!     └── MessageEvent { message_id, user_id, message, sender, inner: MessageKind }
//!         ├── MessageKind::Private(PrivateMessageEvent { sub_type })
//!         └── MessageKind::Group(GroupMessageEvent { group_id, anonymous, sub_type })
//! ```
//!
//! # Usage
//!
//! ```bash
//! cargo run --package echo-bot
//! ```

use alloy::prelude::*;
use alloy_adapter_onebot::{MessageEvent, MessageKind, OneBotAdapter, OneBotBot};
use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::{error, info};

// ============================================================================
// Handler Functions (Axum-style - no macro needed!)
// ============================================================================

/// Example clap-based command for structured argument parsing
///
/// Usage:
/// - /calc add 5 10
/// - /calc multiply 3 4
#[derive(Parser, Debug, Clone)]
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

/// Handler for the "add" subcommand
async fn add_handler(
    ctx: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: Command<CalcCommand>,
) {
    if let CalcOperation::Add { a, b } = &cmd.operation {
        let result = a + b;
        let response = format!("➕ {a} + {b} = {result}");
        if let Err(e) = bot.send(ctx.root.as_ref(), &response).await {
            error!("Failed to send add result: {:?}", e);
        }
    }
}

/// Handler for the "multiply" subcommand
async fn multiply_handler(
    ctx: EventContext<MessageEvent>,
    bot: Arc<OneBotBot>,
    cmd: Command<CalcCommand>,
) {
    if let CalcOperation::Multiply { a, b } = &cmd.operation {
        let result = a * b;
        let response = format!("✖️ {a} × {b} = {result}");
        if let Err(e) = bot.send(ctx.root.as_ref(), &response).await {
            error!("Failed to send multiply result: {:?}", e);
        }
    }
}

/// Logging handler - logs all messages.
///
/// This handler runs for every message event.
async fn logging_handler(ctx: EventContext<MessageEvent>) {
    let msg = ctx.data();
    let nickname = msg.sender.nickname.as_deref().unwrap_or("Unknown");

    match &msg.inner {
        MessageKind::Private(_) => {
            info!(
                "[Private] {} ({}): {}",
                nickname,
                msg.user_id,
                msg.plain_text()
            );
        }
        MessageKind::Group(g) => {
            info!(
                "[Group {}] {} ({}): {}",
                g.group_id,
                nickname,
                msg.user_id,
                msg.plain_text()
            );
        }
    }
}

/// Echo command handler - now sends back the message!
async fn echo_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();
    let text = msg.plain_text();

    // Command prefix already checked by matcher, just extract the content
    if let Some(content) = text.strip_prefix("/echo ")
        && let Err(e) = bot.send(ctx.root.as_ref(), content).await
    {
        error!("Failed to send echo reply: {:?}", e);
    }
}

/// Ping command handler - responds with Pong!
async fn ping_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    // Command already checked by matcher
    if let Err(e) = bot.send(ctx.root.as_ref(), "Pong! 🏓").await {
        error!("Failed to send ping reply: {:?}", e);
    }
}

/// Help command handler - sends help message.
async fn help_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    // Command already checked by matcher
    let help_text = r"╭─────────────────────────────╮
│     Echo Bot - Commands     │
├─────────────────────────────┤
│ /echo <text> - Echo text    │
│ /ping        - Pong!        │
│ /help        - This help    │
│ /info        - Message info │
│ /group       - Group only   │
╰─────────────────────────────╯";

    if let Err(e) = bot.send(ctx.root.as_ref(), help_text).await {
        error!("Failed to send help message: {:?}", e);
    }
}

/// Info command handler - sends message info.
async fn info_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();
    // Command already checked by matcher
    let nickname = msg.sender.nickname.as_deref().unwrap_or("Unknown");

    let info_text = match &msg.inner {
        MessageKind::Private(p) => {
            format!(
                "📋 Message Info\n\
                • Type: Private\n\
                • From: {} ({})\n\
                • Message ID: {}\n\
                • Sub Type: {}",
                nickname, msg.user_id, msg.message_id, p.sub_type
            )
        }
        MessageKind::Group(g) => {
            format!(
                "📋 Message Info\n\
                • Type: Group\n\
                • From: {} ({})\n\
                • Group: {}\n\
                • Message ID: {}\n\
                • Sub Type: {}",
                nickname, msg.user_id, g.group_id, msg.message_id, g.sub_type
            )
        }
    };

    if let Err(e) = bot.send(ctx.root.as_ref(), &info_text).await {
        error!("Failed to send info message: {:?}", e);
    }
}

/// Group-only command handler - responds only in groups.
async fn group_only_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();
    // Command already checked by matcher, but we still need to check group
    if let MessageKind::Group(g) = &msg.inner {
        let nickname = msg.sender.nickname.as_deref().unwrap_or("Unknown");
        let response = format!(
            "✅ This is a group-only command!\n\
                • Group ID: {}\n\
                • User: {} ({})",
            g.group_id, nickname, msg.user_id
        );

        if let Err(e) = bot.send(ctx.root.as_ref(), &response).await {
            error!("Failed to send group-only response: {:?}", e);
        }
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
    // Register Matchers using convenience functions
    // ========================================================================

    runtime
        .register_matchers(vec![
            // Logging - runs for all message events, does NOT block
            on_message()
                .block(false) // Don't block - let other matchers also process
                .handler(logging_handler),
            // Command handlers - these use on_command() which auto-prepends "/"
            on_command("echo").handler(echo_handler),
            on_command("ping").handler(ping_handler),
            on_command("help").handler(help_handler),
            on_command("info").handler(info_handler),
            on_command("group").handler(group_only_handler),
            // Structured command handler using clap with subcommand routing
            // Demonstrates automatic help (-h) and error messages
            // Example: /calc add 5 10
            //          /calc multiply 3 4
            //          /calc -h (shows help)
            on_command_struct::<CalcCommand>("/calc")
                .reply_help(true) // Automatic help message on -h or --help
                .reply_error(true) // Automatic error messages on parse failure
                .router()
                .route("add", add_handler)
                .route("multiply", multiply_handler)
                .build(),
        ])
        .await;

    // Run the runtime
    runtime.run().await?;

    Ok(())
}
