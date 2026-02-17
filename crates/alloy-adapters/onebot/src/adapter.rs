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
use crate::model::event::{LifecycleEvent, parse_onebot_event};
use alloy_core::{
    Adapter, AdapterBridge, AdapterResult, BoxedBot, BoxedEvent, ClientConfig, ConfigurableAdapter,
    ConnectionHandle, ConnectionInfo, TransportError, TransportResult,
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
    async fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String> {
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
        OneBotBot::new(bot_id, connection)
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
            if let Ok(onebot_bot) = Arc::downcast::<OneBotBot>(bot.clone().as_any()) {
                onebot_bot.handle_response(&value).await;
                trace!(bot_id = %bot_id, echo = ?value.get("echo"), "Handled API response");
            } else {
                warn!(bot_id = %bot_id, "Bot is not OneBotBot, cannot handle response");
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

        // Log at appropriate level
        let event_name = boxed_event.event_name();
        if event_name == "onebot.meta_event.heartbeat" {
            trace!(bot_id = %bot_id, "Received heartbeat");
        } else if event_name.starts_with("onebot.meta_event.lifecycle") {
            if let Some(lc) = boxed_event.as_any().downcast_ref::<LifecycleEvent>() {
                debug!(bot_id = %bot_id, sub_type = ?lc.sub_type, "Received lifecycle event");
            }
        } else {
            info!(
                bot_id = %bot_id,
                event = %event_name,
                "Received event"
            );
        }

        Some(boxed_event)
    }

    async fn on_start(&self, bridge: Arc<AdapterBridge>) -> AdapterResult<()> {
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
                    if let Some(ws_server) = bridge.transport().ws_server() {
                        let addr = ws_config.bind_addr();
                        info!(
                            name = %ws_config.name,
                            addr = %addr,
                            path = %ws_config.path,
                            "Starting WebSocket server"
                        );
                        let handle = ws_server
                            .listen(&addr, &ws_config.path, bridge.clone())
                            .await?;
                        bridge.add_listener(handle).await;
                    } else {
                        warn!(
                            "WebSocket server capability not available, skipping ws-server config"
                        );
                    }
                }

                ConnectionConfig::WsClient(ws_config) => {
                    if let Some(ws_client) = bridge.transport().ws_client() {
                        let token = ws_config
                            .access_token
                            .as_ref()
                            .or(self.config.default_access_token.as_ref())
                            .filter(|t| !t.is_empty());
                        info!(
                            name = %ws_config.name,
                            url = %ws_config.url,
                            has_token = token.is_some(),
                            "Connecting to WebSocket server"
                        );
                        let config = if let Some(t) = token {
                            ClientConfig::default().with_token(t)
                        } else {
                            ClientConfig::default()
                        };
                        let handle = ws_client
                            .connect(&ws_config.url, bridge.clone(), config)
                            .await?;
                        bridge.add_connection(handle).await;
                    } else {
                        warn!(
                            "WebSocket client capability not available, skipping ws-client config"
                        );
                    }
                }

                ConnectionConfig::HttpServer(http_config) => {
                    if let Some(http_server) = bridge.transport().http_server() {
                        let addr = http_config.bind_addr();
                        info!(
                            name = %http_config.name,
                            addr = %addr,
                            path = %http_config.path,
                            "Starting HTTP webhook server"
                        );
                        let handle = http_server
                            .listen(&addr, &http_config.path, bridge.clone())
                            .await?;
                        bridge.add_listener(handle).await;
                    } else {
                        warn!("HTTP server capability not available, skipping http-server config");
                    }
                }

                ConnectionConfig::HttpClient(http_config) => {
                    // HTTP client bots are created in the transport layer.
                    // They can send API calls but don't receive events via this connection.
                    // Events come from a separate HTTP server or WS connection.
                    if let Some(http_client) = bridge.transport().http_client() {
                        let bot_id = http_config.bot_id.clone();
                        let api_url = http_config.api_url.clone();
                        let access_token = http_config
                            .access_token
                            .as_ref()
                            .or(self.config.default_access_token.as_ref())
                            .cloned();

                        info!(
                            name = %http_config.name,
                            bot_id = %bot_id,
                            url = %api_url,
                            has_token = access_token.is_some(),
                            "Starting HTTP API client bot (send-only)"
                        );

                        let handle = http_client
                            .start_client(&bot_id, &api_url, access_token, bridge.clone())
                            .await?;
                        bridge.add_connection(handle).await;
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

    fn from_config(config: Self::Config) -> AdapterResult<Arc<Self>> {
        Ok(Arc::new(Self { config }))
    }
}
