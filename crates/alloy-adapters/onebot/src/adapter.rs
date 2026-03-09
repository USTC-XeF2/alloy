//! OneBot v11 adapter for the Alloy framework.
//!
//! This module provides the main adapter that bridges OneBot v11 implementations
//! with the Alloy event system.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{trace, warn};

use crate::bot::OneBotBot;
use crate::config::{ConnectionConfig, OneBotConfig};
use crate::model::event::parse_onebot_event;
use alloy_core::{
    Adapter, AdapterContext, AdapterResult, BoxedBot, BoxedEvent, ConfigurableAdapter,
    ConnectionHandle, ConnectionInfo, HttpClientConfig, HttpServerConfig, TransportError,
    TransportResult, WsClientConfig, WsServerConfig,
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

        Ok(bot_id)
    }

    fn create_bot(&self, bot_id: &str, connection: &ConnectionHandle) -> BoxedBot {
        Arc::new(OneBotBot::new(bot_id, connection))
    }

    async fn on_message(&self, bot: &BoxedBot, data: &[u8]) -> Option<BoxedEvent> {
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
            if let Ok(onebot_bot) = bot.clone().downcast_arc::<OneBotBot>() {
                onebot_bot.handle_response(&value);
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

    async fn on_start(&self, ctx: Box<dyn AdapterContext>) -> AdapterResult<()> {
        if self.config.connections.is_empty() {
            warn!("No connections in OneBot adapter configuration");
            return Ok(());
        }

        for conn_config in &self.config.connections {
            match conn_config {
                ConnectionConfig::WsServer(ws_config) => {
                    if let Some(ws_server) = ctx.transport().ws_server() {
                        let mut config =
                            WsServerConfig::new(&ws_config.host, ws_config.port, &ws_config.path);
                        if let Some(t) = &ws_config.access_token {
                            config = config.with_token(t);
                        }
                        ws_server(config, ctx.as_connection_handler()).await?;
                    } else {
                        warn!(
                            "WebSocket server capability not available, skipping ws-server config"
                        );
                    }
                }

                ConnectionConfig::WsClient(ws_config) => {
                    if let Some(ws_client) = ctx.transport().ws_client() {
                        let mut config = WsClientConfig::new(&ws_config.url);
                        if let Some(t) = ws_config.access_token.as_ref().filter(|t| !t.is_empty()) {
                            config = config.with_token(t);
                        }
                        ws_client(config, ctx.as_connection_handler()).await?;
                    } else {
                        warn!(
                            "WebSocket client capability not available, skipping ws-client config"
                        );
                    }
                }

                ConnectionConfig::HttpServer(http_config) => {
                    if let Some(http_server) = ctx.transport().http_server() {
                        let mut config = HttpServerConfig::new(
                            &http_config.host,
                            http_config.port,
                            &http_config.path,
                        );
                        if let Some(s) = &http_config.secret {
                            config = config.with_secret(s);
                        }
                        http_server(config, ctx.as_connection_handler()).await?;
                    } else {
                        warn!("HTTP server capability not available, skipping http-server config");
                    }
                }

                ConnectionConfig::HttpClient(http_config) => {
                    if let Some(http_client) = ctx.transport().http_client() {
                        let mut client_config =
                            HttpClientConfig::new(&http_config.bot_id, &http_config.api_url);
                        if let Some(token) = http_config.access_token.as_ref() {
                            client_config = client_config.with_token(token);
                        }

                        http_client(client_config, ctx.as_connection_handler()).await?;
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
    const NAME: &'static str = "onebot";
    type Config = OneBotConfig;

    fn from_config(config: Self::Config) -> Self {
        Self { config }
    }
}
