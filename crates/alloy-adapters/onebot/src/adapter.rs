//! OneBot v11 adapter for the Alloy framework.
//!
//! This module provides the main adapter that bridges OneBot v11 implementations
//! with the Alloy event system.
//!
//! # Capability-Based Design
//!
//! The adapter uses capability discovery to find available transports:
//!
//! ```rust,ignore
//! use alloy_runtime::AlloyRuntime;
//! use alloy_adapter_onebot::OneBotAdapter;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = AlloyRuntime::load_config()?;
//!     runtime.register_adapter(OneBotAdapter::new()).await;
//!     runtime.run().await
//! }
//! ```

use std::sync::Arc;

use alloy_core::{
    AdapterContext, BoxedConnectionHandler, BoxedEvent, ClientConfig, ConnectionHandle,
    ConnectionHandler, ConnectionInfo, Event,
};
use async_trait::async_trait;
use tracing::{debug, info, trace, warn};

use crate::bot::OneBotBot;
use crate::model::event::{MessageEvent, MessageKind, MetaEventKind, OneBotEvent, OneBotEventKind};
use crate::traits::{GroupEvent, MemberRole, PrivateEvent};

/// The OneBot v11 adapter.
///
/// This adapter implements the `alloy_core::Adapter` trait and uses
/// the capability discovery pattern to find available transports.
pub struct OneBotAdapter {
    /// Default WebSocket server address.
    ws_server_addr: String,
    /// Default WebSocket server path.
    ws_server_path: String,
    /// Default WebSocket client URL.
    ws_client_url: Option<String>,
    /// Access token for authentication.
    access_token: Option<String>,
}

impl OneBotAdapter {
    /// Creates a new OneBot adapter with default settings.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            ws_server_addr: "0.0.0.0:8080".to_string(),
            ws_server_path: "/onebot/v11/ws".to_string(),
            ws_client_url: None,
            access_token: None,
        })
    }

    /// Creates an adapter builder.
    pub fn builder() -> OneBotAdapterBuilder {
        OneBotAdapterBuilder::default()
    }
}

impl Default for OneBotAdapter {
    fn default() -> Self {
        Self {
            ws_server_addr: "0.0.0.0:8080".to_string(),
            ws_server_path: "/onebot/v11/ws".to_string(),
            ws_client_url: None,
            access_token: None,
        }
    }
}

/// Builder for `OneBotAdapter`.
#[derive(Default)]
pub struct OneBotAdapterBuilder {
    ws_server_addr: Option<String>,
    ws_server_path: Option<String>,
    ws_client_url: Option<String>,
    access_token: Option<String>,
}

impl OneBotAdapterBuilder {
    /// Sets the WebSocket server listen address.
    pub fn ws_server_addr(mut self, addr: impl Into<String>) -> Self {
        self.ws_server_addr = Some(addr.into());
        self
    }

    /// Sets the WebSocket server path.
    pub fn ws_server_path(mut self, path: impl Into<String>) -> Self {
        self.ws_server_path = Some(path.into());
        self
    }

    /// Sets the WebSocket client URL to connect to.
    pub fn ws_client_url(mut self, url: impl Into<String>) -> Self {
        self.ws_client_url = Some(url.into());
        self
    }

    /// Sets the access token for authentication.
    pub fn access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Builds the adapter.
    pub fn build(self) -> Arc<OneBotAdapter> {
        Arc::new(OneBotAdapter {
            ws_server_addr: self
                .ws_server_addr
                .unwrap_or_else(|| "0.0.0.0:8080".to_string()),
            ws_server_path: self
                .ws_server_path
                .unwrap_or_else(|| "/onebot/v11/ws".to_string()),
            ws_client_url: self.ws_client_url,
            access_token: self.access_token,
        })
    }
}

#[async_trait]
impl alloy_core::Adapter for OneBotAdapter {
    fn name(&self) -> &'static str {
        "onebot"
    }

    async fn on_start(&self, ctx: &mut AdapterContext) -> anyhow::Result<()> {
        // Check for WebSocket server capability
        if let Some(ws_server) = ctx.transport().ws_server() {
            info!(addr = %self.ws_server_addr, path = %self.ws_server_path, "Starting WebSocket server");
            let handler = Arc::new(OneBotConnectionHandler::new(Arc::clone(ctx.bot_manager())));
            let handle = ws_server
                .listen(&self.ws_server_addr, &self.ws_server_path, handler)
                .await?;
            ctx.add_listener(handle);
        }

        // Check for WebSocket client capability
        if let Some(ws_client) = ctx.transport().ws_client()
            && let Some(url) = &self.ws_client_url
        {
            info!(url = %url, "Connecting to WebSocket server");
            let handler = Arc::new(OneBotConnectionHandler::new(Arc::clone(ctx.bot_manager())));
            let config = if let Some(token) = &self.access_token {
                ClientConfig::default().with_token(token)
            } else {
                ClientConfig::default()
            };
            let handle = ws_client.connect(url, handler, config).await?;
            ctx.add_connection(handle);
        }

        Ok(())
    }

    async fn on_shutdown(&self, _ctx: &mut AdapterContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_connection_handler(&self) -> BoxedConnectionHandler {
        panic!("create_connection_handler should not be called directly. Use on_start instead.")
    }

    fn parse_event(&self, data: &[u8]) -> anyhow::Result<Option<BoxedEvent>> {
        let raw = str::from_utf8(data)?;

        // Use OneBotEvent for automatic parsing - the whole event is boxed
        let event = OneBotEvent::parse(raw)?;

        // Log meta events at appropriate level
        if let OneBotEventKind::MetaEvent(meta) = &event.inner
            && let MetaEventKind::Heartbeat(_) = &meta.inner
        {
            trace!("Heartbeat from bot {}", event.self_id);
        }

        Ok(Some(BoxedEvent::new(event)))
    }

    fn clone_adapter(&self) -> Arc<dyn alloy_core::Adapter> {
        OneBotAdapter::new()
    }
}

/// Connection handler for OneBot connections.
struct OneBotConnectionHandler {
    bot_manager: Arc<alloy_core::BotManager>,
}

impl OneBotConnectionHandler {
    fn new(bot_manager: Arc<alloy_core::BotManager>) -> Self {
        Self { bot_manager }
    }
}

#[async_trait]
impl ConnectionHandler for OneBotConnectionHandler {
    async fn on_connect(&self, conn_info: ConnectionInfo) -> String {
        // OneBot v11 uses X-Self-ID header to identify the bot
        // Headers are stored in lowercase in metadata
        let bot_id = conn_info
            .metadata
            .get("x-self-id")
            .cloned()
            .or_else(|| conn_info.remote_addr.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        info!(
            bot_id = %bot_id,
            remote_addr = ?conn_info.remote_addr,
            "OneBot connection established"
        );

        bot_id
    }

    async fn on_ready(&self, bot_id: &str, connection: ConnectionHandle) {
        // Create and register the OneBotBot instance with both connection and bot
        let bot = OneBotBot::new(bot_id, connection.clone());

        // Register with bot instance
        if let Err(e) = self
            .bot_manager
            .register_with_bot(bot_id.to_string(), connection, "onebot".to_string(), bot)
            .await
        {
            warn!(bot_id = %bot_id, error = %e, "Failed to register bot instance");
        } else {
            debug!(bot_id = %bot_id, "OneBotBot instance registered");
        }
    }

    async fn on_message(&self, bot_id: &str, data: &[u8]) -> Option<BoxedEvent> {
        // Parse the message as JSON first
        let raw = match str::from_utf8(data) {
            Ok(s) => s,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Invalid UTF-8 in message");
                return None;
            }
        };

        // Try to parse as JSON to check if it's an API response
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            // Check if this is an API response (has "echo" field)
            if value.get("echo").is_some() {
                // This is an API response, forward to the bot instance
                if let Some(bot) = self.bot_manager.get_bot(bot_id).await {
                    // Try to downcast to OneBotBot using as_any()
                    if let Ok(onebot_bot) = Arc::downcast::<OneBotBot>(bot.as_any()) {
                        onebot_bot.handle_response(&value).await;
                        trace!(bot_id = %bot_id, echo = ?value.get("echo"), "Handled API response");
                    } else {
                        warn!(bot_id = %bot_id, "Bot is not OneBotBot, cannot handle response");
                    }
                } else {
                    warn!(bot_id = %bot_id, "Bot not found for API response");
                }
                return None; // API responses are not events
            }
        }

        // Parse as event
        let event = match OneBotEvent::parse(raw) {
            Ok(e) => e,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, raw_data = %raw, "Failed to parse event raw data");
                return None;
            }
        };

        // Log at appropriate level
        match &event.inner {
            OneBotEventKind::MetaEvent(meta) => match &meta.inner {
                MetaEventKind::Heartbeat(_) => {
                    trace!(bot_id = %bot_id, "Received heartbeat");
                }
                MetaEventKind::Lifecycle(lc) => {
                    debug!(bot_id = %bot_id, sub_type = ?lc.sub_type, "Received lifecycle event");
                }
            },
            _ => {
                info!(
                    bot_id = %bot_id,
                    event = %event.event_name(),
                    "Received event"
                );
            }
        }

        let boxed_event = BoxedEvent::new(event);
        // Dispatch event through bot manager (requires bot)
        self.bot_manager.dispatch_event(boxed_event.clone()).await;
        Some(boxed_event)
    }

    async fn on_disconnect(&self, bot_id: &str) {
        // Clear pending calls before unregistering
        // This ensures all waiting API callers receive NotConnected errors
        if let Some(bot) = self.bot_manager.get_bot(bot_id).await {
            if let Ok(onebot_bot) = Arc::downcast::<OneBotBot>(bot.as_any()) {
                onebot_bot.clear_pending_calls().await;
            }
        }

        // Unregister the bot
        self.bot_manager.unregister(bot_id).await;
        info!(bot_id = %bot_id, "OneBot connection closed");
    }

    async fn on_error(&self, bot_id: &str, error: &str) {
        warn!(bot_id = %bot_id, error = %error, "OneBot connection error");
    }
}

// ============================================================================
// Group/Private event trait implementations
// ============================================================================

impl GroupEvent for MessageEvent {
    fn get_group_id(&self) -> &str {
        match &self.inner {
            MessageKind::Group(g) => Box::leak(g.group_id.to_string().into_boxed_str()),
            MessageKind::Private(_) => "",
        }
    }

    fn get_sender_role(&self) -> MemberRole {
        match self.sender.role.as_deref() {
            Some("owner") => MemberRole::Owner,
            Some("admin") => MemberRole::Admin,
            _ => MemberRole::Member,
        }
    }

    fn get_sender_id(&self) -> &str {
        Box::leak(self.user_id.to_string().into_boxed_str())
    }

    fn get_sender_nickname(&self) -> Option<&str> {
        self.sender.nickname.as_deref()
    }
}

impl PrivateEvent for MessageEvent {
    fn get_user_id(&self) -> &str {
        Box::leak(self.user_id.to_string().into_boxed_str())
    }

    fn get_user_nickname(&self) -> Option<&str> {
        self.sender.nickname.as_deref()
    }
}
