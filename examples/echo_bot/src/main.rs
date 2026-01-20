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

use alloy::EventContext;
use alloy::alloy_runtime::AlloyRuntime;
use alloy::{Bot, prelude::*};
use alloy_adapter_onebot::{MessageEvent, MessageKind, OneBotAdapter, OneBotBot};
use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info};

// ============================================================================
// Handler Functions (Axum-style - no macro needed!)
// ============================================================================

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

    if let Some(content) = text.strip_prefix("/echo ")
        && let Err(e) = bot.send(ctx.root.as_ref(), content).await
    {
        error!("Failed to send echo reply: {:?}", e);
    }
}

/// Ping command handler - responds with Pong!
async fn ping_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();

    if msg.plain_text().trim() == "/ping"
        && let Err(e) = bot.send(ctx.root.as_ref(), "Pong! 🏓").await
    {
        error!("Failed to send ping reply: {:?}", e);
    }
}

/// Help command handler - sends help message.
async fn help_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    if ctx.data().plain_text().trim() == "/help" {
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
}

/// Info command handler - sends message info.
async fn info_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();

    if msg.plain_text().trim() == "/info" {
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
}

/// Group-only command handler - responds only in groups.
async fn group_only_handler(ctx: EventContext<MessageEvent>, bot: Arc<OneBotBot>) {
    let msg = ctx.data();

    if msg.plain_text().trim() == "/group"
        && let MessageKind::Group(g) = &msg.inner
    {
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
    // Custom logging (must be done BEFORE AlloyRuntime is created)
    // AlloyRuntime::init_logging_custom(|| {
    //     LoggingBuilder::new()
    //         .with_level(Level::DEBUG)
    //         .directive("alloy_core=info")
    //         .directive("alloy_transport=info")
    //         .with_span_events(SpanEvents::LIFECYCLE)
    //         .with_target(true)
    //         .init();
    // });

    // Create the runtime
    let runtime = AlloyRuntime::new();

    // Register the OneBot adapter with custom settings
    let adapter = OneBotAdapter::builder()
        .ws_server_addr("127.0.0.1:8080")
        .ws_server_path("/onebot/v11/ws")
        .build();
    runtime.register_adapter(adapter).await;

    // ========================================================================
    // Register Matchers
    // ========================================================================

    // Matcher 1: Logging - runs for all message events, does NOT block
    runtime
        .register_matcher(
            Matcher::new()
                .name("logging")
                .on::<MessageEvent>()
                .block(false) // Don't block - let other matchers also process
                .handler(logging_handler),
        )
        .await;

    // Matcher 2: Commands - handles all command handlers, blocks after processing
    runtime
        .register_matcher(
            Matcher::new()
                .name("commands")
                .on::<MessageEvent>()
                .block(false) // Don't block - commands don't consume all messages
                .handler(echo_handler)
                .handler(ping_handler)
                .handler(help_handler)
                .handler(info_handler)
                .handler(group_only_handler),
        )
        .await;

    // Run the runtime
    runtime.run().await?;

    Ok(())
}
