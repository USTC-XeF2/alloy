//! OneBot v11 adapter for the Alloy framework.
//!
//! This module provides the main adapter that bridges OneBot v11 implementations
//! with the Alloy event system.
//!
//! # Configuration-Based Usage (Recommended)
//!
//! The adapter reads its configuration from `alloy.yaml`:
//!
//! ```yaml
//! adapters:
//!   onebot:
//!     connections:
//!       - type: ws-server
//!         host: 0.0.0.0
//!         port: 8080
//!         path: /onebot/v11/ws
//!       - type: ws-client
//!         url: ws://127.0.0.1:6700/ws
//!         access_token: ${BOT_TOKEN:-}
//! ```
//!
//! ```rust,ignore
//! use alloy_runtime::AlloyRuntime;
//! use alloy_adapter_onebot::OneBotAdapter;
//!
//! let runtime = AlloyRuntime::new();
//! // Adapter auto-created from config
//! runtime.run().await?;
//! ```
//!
//! # Programmatic Usage
//!
//! ```rust,ignore
//! let adapter = OneBotAdapter::from_config(config);
//! // Or build manually
//! let adapter = OneBotAdapter::builder()
//!     .ws_server("0.0.0.0:8080", "/ws")
//!     .ws_client("ws://localhost:6700/ws", Some("token"))
//!     .build();
//! ```

use std::sync::Arc;

use alloy_core::{
    AdapterContext, BoxedConnectionHandler, BoxedEvent, ClientConfig, ConnectionHandle,
    ConnectionHandler, ConnectionInfo,
};
use async_trait::async_trait;
use tracing::{debug, info, trace, warn};

use crate::bot::OneBotBot;
use crate::config::{ConnectionConfig, OneBotConfig, WsClientConfig, WsServerConfig};
use crate::model::event::{LifecycleEvent, parse_onebot_event};

/// The OneBot v11 adapter.
///
/// Supports multiple simultaneous connections of different types.
#[derive(Default)]
pub struct OneBotAdapter {
    /// Adapter configuration.
    config: OneBotConfig,
}

impl OneBotAdapter {
    /// Creates an adapter builder.
    pub fn builder() -> OneBotAdapterBuilder {
        OneBotAdapterBuilder::default()
    }

    /// Returns the adapter configuration.
    pub fn config(&self) -> &OneBotConfig {
        &self.config
    }

    /// Resolves the access token for a connection.
    fn resolve_token(&self, connection_token: Option<&String>) -> Option<String> {
        connection_token
            .cloned()
            .or_else(|| self.config.default_access_token.clone())
            .filter(|t| !t.is_empty())
    }
}

/// Builder for `OneBotAdapter`.
///
/// Allows programmatic construction of the adapter.
#[derive(Default)]
pub struct OneBotAdapterBuilder {
    connections: Vec<ConnectionConfig>,
    default_access_token: Option<String>,
    auto_reconnect: bool,
}

impl OneBotAdapterBuilder {
    /// Adds a WebSocket server connection.
    pub fn ws_server(mut self, addr: impl Into<String>, path: impl Into<String>) -> Self {
        let addr = addr.into();
        let (host, port) = parse_addr(&addr);
        self.connections
            .push(ConnectionConfig::WsServer(WsServerConfig {
                name: format!("ws-server-{}", self.connections.len()),
                enabled: true,
                host,
                port,
                path: path.into(),
                access_token: None,
            }));
        self
    }

    /// Adds a WebSocket server connection with a token.
    pub fn ws_server_with_token(
        mut self,
        addr: impl Into<String>,
        path: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        let addr = addr.into();
        let (host, port) = parse_addr(&addr);
        self.connections
            .push(ConnectionConfig::WsServer(WsServerConfig {
                name: format!("ws-server-{}", self.connections.len()),
                enabled: true,
                host,
                port,
                path: path.into(),
                access_token: Some(token.into()),
            }));
        self
    }

    /// Adds a WebSocket client connection.
    pub fn ws_client(mut self, url: impl Into<String>, token: Option<String>) -> Self {
        self.connections
            .push(ConnectionConfig::WsClient(WsClientConfig {
                name: format!("ws-client-{}", self.connections.len()),
                enabled: true,
                url: url.into(),
                access_token: token,
                auto_reconnect: true,
                reconnect_delay_ms: 5000,
            }));
        self
    }

    /// Sets the default access token for all connections.
    pub fn default_access_token(mut self, token: impl Into<String>) -> Self {
        self.default_access_token = Some(token.into());
        self
    }

    /// Enables auto-reconnect for client connections.
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Builds the adapter.
    pub fn build(self) -> Arc<OneBotAdapter> {
        // If no connections specified, use default WsServer
        let connections = if self.connections.is_empty() {
            vec![ConnectionConfig::WsServer(WsServerConfig::default())]
        } else {
            self.connections
        };

        Arc::new(OneBotAdapter {
            config: OneBotConfig {
                connections,
                default_access_token: self.default_access_token,
                auto_reconnect: self.auto_reconnect,
                heartbeat_interval_secs: 30,
            },
        })
    }
}

/// Parses an address string like "0.0.0.0:8080" into (host, port).
fn parse_addr(addr: &str) -> (String, u16) {
    if let Some((host, port_str)) = addr.rsplit_once(':') {
        let port = port_str.parse().unwrap_or(8080);
        (host.to_string(), port)
    } else {
        (addr.to_string(), 8080)
    }
}

#[async_trait]
impl alloy_core::Adapter for OneBotAdapter {
    fn name() -> &'static str {
        "onebot"
    }

    async fn on_start(&self, ctx: &mut AdapterContext) -> anyhow::Result<()> {
        let handler: Arc<dyn ConnectionHandler> =
            Arc::new(OneBotConnectionHandler::new(Arc::clone(ctx.bot_manager())));

        let enabled_count = self.config.enabled_count();
        if enabled_count == 0 {
            warn!("No enabled connections in OneBot adapter configuration");
            return Ok(());
        }

        info!(
            enabled = enabled_count,
            total = self.config.connections.len(),
            "Starting OneBot adapter connections"
        );

        for conn_config in self.config.enabled_connections() {
            match conn_config {
                ConnectionConfig::WsServer(ws_config) => {
                    if let Some(ws_server) = ctx.transport().ws_server() {
                        let addr = ws_config.bind_addr();
                        info!(
                            name = %ws_config.name,
                            addr = %addr,
                            path = %ws_config.path,
                            "Starting WebSocket server"
                        );
                        let handle = ws_server
                            .listen(&addr, &ws_config.path, Arc::clone(&handler))
                            .await?;
                        ctx.add_listener(handle);
                    } else {
                        warn!(
                            "WebSocket server capability not available, skipping ws-server config"
                        );
                    }
                }

                ConnectionConfig::WsClient(ws_config) => {
                    if let Some(ws_client) = ctx.transport().ws_client() {
                        let token = self.resolve_token(ws_config.access_token.as_ref());
                        info!(
                            name = %ws_config.name,
                            url = %ws_config.url,
                            has_token = token.is_some(),
                            "Connecting to WebSocket server"
                        );
                        let config = if let Some(ref t) = token {
                            ClientConfig::default().with_token(t)
                        } else {
                            ClientConfig::default()
                        };
                        let handle = ws_client
                            .connect(&ws_config.url, Arc::clone(&handler), config)
                            .await?;
                        ctx.add_connection(handle);
                    } else {
                        warn!(
                            "WebSocket client capability not available, skipping ws-client config"
                        );
                    }
                }

                ConnectionConfig::HttpServer(http_config) => {
                    if let Some(http_server) = ctx.transport().http_server() {
                        let addr = http_config.bind_addr();
                        info!(
                            name = %http_config.name,
                            addr = %addr,
                            path = %http_config.path,
                            "Starting HTTP webhook server"
                        );
                        // TODO: Implement HTTP server handler
                        let _ = http_server;
                        warn!("HTTP server not yet implemented");
                    } else {
                        warn!("HTTP server capability not available, skipping http-server config");
                    }
                }

                ConnectionConfig::HttpClient(http_config) => {
                    if let Some(http_client) = ctx.transport().http_client() {
                        info!(
                            name = %http_config.name,
                            url = %http_config.api_url,
                            "Configuring HTTP API client"
                        );
                        // TODO: Implement HTTP client
                        let _ = http_client;
                        warn!("HTTP client not yet implemented");
                    } else {
                        warn!("HTTP client capability not available, skipping http-client config");
                    }
                }
            }
        }

        Ok(())
    }

    async fn on_shutdown(&self, _ctx: &mut AdapterContext) -> anyhow::Result<()> {
        info!("OneBot adapter shutting down");
        Ok(())
    }

    fn create_connection_handler(&self) -> BoxedConnectionHandler {
        panic!("create_connection_handler should not be called directly. Use on_start instead.")
    }

    fn parse_event(&self, data: &[u8]) -> anyhow::Result<Option<BoxedEvent>> {
        let raw = std::str::from_utf8(data)?;
        let event = parse_onebot_event(raw)?;

        // Log heartbeat at trace level
        if event.event_name() == "onebot.meta_event.heartbeat" {
            trace!("Heartbeat from bot {:?}", event.bot_id());
        }

        Ok(Some(event))
    }

    fn clone_adapter(&self) -> Arc<dyn alloy_core::Adapter> {
        Arc::new(Self {
            config: self.config.clone(),
        })
    }
}

impl alloy_core::ConfigurableAdapter for OneBotAdapter {
    type Config = OneBotConfig;

    fn from_config(config: Self::Config) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self { config }))
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

        // Parse as event using the central dispatcher
        let boxed_event = match parse_onebot_event(raw) {
            Ok(e) => e,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, raw_data = %raw, "Failed to parse event raw data");
                return None;
            }
        };

        // Log at appropriate level
        let event_name = boxed_event.event_name();
        if event_name == "onebot.meta_event.heartbeat" {
            trace!(bot_id = %bot_id, "Received heartbeat");
        } else if event_name.starts_with("onebot.meta_event.lifecycle") {
            // Downcast to get lifecycle sub_type for logging
            if let Some(lc) = boxed_event.downcast_ref::<LifecycleEvent>() {
                debug!(bot_id = %bot_id, sub_type = ?lc.sub_type, "Received lifecycle event");
            }
        } else {
            info!(
                bot_id = %bot_id,
                event = %event_name,
                "Received event"
            );
        }

        // Dispatch event through bot manager (requires bot)
        self.bot_manager.dispatch_event(boxed_event.clone()).await;
        Some(boxed_event)
    }

    async fn on_disconnect(&self, bot_id: &str) {
        // Clear pending calls before unregistering
        // This ensures all waiting API callers receive NotConnected errors
        if let Some(bot) = self.bot_manager.get_bot(bot_id).await
            && let Ok(onebot_bot) = Arc::downcast::<OneBotBot>(bot.as_any())
        {
            onebot_bot.clear_pending_calls().await;
        }

        // Unregister the bot
        self.bot_manager.unregister(bot_id).await;
        info!(bot_id = %bot_id, "OneBot connection closed");
    }

    async fn on_error(&self, bot_id: &str, error: &str) {
        warn!(bot_id = %bot_id, error = %error, "OneBot connection error");
    }
}
