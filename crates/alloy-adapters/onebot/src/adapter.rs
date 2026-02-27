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

use async_trait::async_trait;
use tracing::{debug, info, trace, warn};

use crate::bot::OneBotBot;
use crate::config::{ConnectionConfig, OneBotConfig};
use crate::model::event::parse_onebot_event;
use alloy_core::{
    Adapter, AdapterContext, AdapterResult, BoxedBot, BoxedEvent, ConfigurableAdapter,
    ConnectionHandle, ConnectionInfo, HttpClientConfig, TransportError, TransportResult,
    WsClientConfig,
};

/// The OneBot v11 adapter.
///
/// Supports multiple simultaneous connections of different types.
#[derive(Default)]
pub struct OneBotAdapter {
    /// Adapter configuration.
    config: OneBotConfig,
}

#[async_trait]
impl Adapter for OneBotAdapter {
    fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String> {
        // OneBot v11 uses X-Self-ID header to identify the bot
        let bot_id = conn_info
            .metadata
            .get("x-self-id")
            .cloned()
            .ok_or_else(|| TransportError::BotIdMissing {
                reason: format!(
                    "x-self-id header not found in connection metadata. Remote: {:?}",
                    conn_info.remote_addr
                ),
            })?;

        info!(
            bot_id = %bot_id,
            remote_addr = ?conn_info.remote_addr,
            "OneBot connection established"
        );

        Ok(bot_id)
    }

    fn create_bot(&self, bot_id: &str, connection: ConnectionHandle) -> BoxedBot {
        Arc::new(OneBotBot::new(bot_id, connection))
    }

    async fn parse_event(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent> {
        let bot_id = bot.id();

        // Parse the message as JSON first
        let raw = match str::from_utf8(data) {
            Ok(s) => s,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Invalid UTF-8 in message");
                return None;
            }
        };

        // Try to parse as JSON to check if it's an API response
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw)
            && value.get("echo").is_some()
        {
            if let Ok(onebot_bot) = Arc::downcast::<OneBotBot>(bot.clone().as_any()) {
                onebot_bot.api_caller.on_incoming_response(&value);
                trace!(bot_id = %bot_id, echo = ?value.get("echo"), "Handled API response");
            }
            return None; // API responses are not events
        }

        // Parse as event
        let boxed_event = match parse_onebot_event(raw) {
            Ok(e) => e,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, raw_data = %raw, "Failed to parse event raw data");
                return None;
            }
        };

        Some(boxed_event)
    }

    async fn on_start(&self, ctx: Arc<dyn AdapterContext>) -> AdapterResult<()> {
        let enabled_count = self.config.enabled_count();
        if enabled_count == 0 {
            warn!("No enabled connections in OneBot adapter configuration");
            return Ok(());
        }

        debug!(
            enabled = enabled_count,
            total = self.config.connections.len(),
            "Starting OneBot adapter connections"
        );

        for conn_config in self.config.enabled_connections() {
            match conn_config {
                ConnectionConfig::WsServer(ws_config) => {
                    if let Some(ws_server) = ctx.transport().ws_server() {
                        let addr = ws_config.bind_addr();
                        let handle = ws_server(
                            addr,
                            ws_config.path.clone(),
                            ctx.clone().as_connection_handler(),
                        )
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
                        let token = ws_config
                            .access_token
                            .as_ref()
                            .or(self.config.default_access_token.as_ref())
                            .filter(|t| !t.is_empty());
                        let mut config = WsClientConfig::new(&ws_config.url);
                        if let Some(t) = token {
                            config = config.with_token(t);
                        }
                        let handle = ws_client(config, ctx.clone().as_connection_handler()).await?;
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
                        let handle = http_server(
                            addr,
                            http_config.path.clone(),
                            ctx.clone().as_connection_handler(),
                        )
                        .await?;
                        ctx.add_listener(handle);
                    } else {
                        warn!("HTTP server capability not available, skipping http-server config");
                    }
                }

                ConnectionConfig::HttpClient(http_config) => {
                    if let Some(http_client) = ctx.transport().http_client() {
                        let bot_id = http_config.bot_id.clone();
                        let access_token = http_config
                            .access_token
                            .as_ref()
                            .or(self.config.default_access_token.as_ref())
                            .cloned();

                        let mut client_config = HttpClientConfig::new(&http_config.api_url);
                        if let Some(token) = access_token {
                            client_config = client_config.with_token(token);
                        }

                        let handle =
                            http_client(bot_id, client_config, ctx.clone().as_connection_handler())
                                .await?;
                        ctx.add_connection(handle);
                    } else {
                        warn!("HTTP client capability not available, skipping http-client config");
                    }
                }
            }
        }

        Ok(())
    }
}

impl ConfigurableAdapter for OneBotAdapter {
    type Config = OneBotConfig;

    fn name() -> &'static str {
        "onebot"
    }

    fn from_config(config: Self::Config) -> Self {
        Self { config }
    }
}
