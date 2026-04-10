//! OneBot v11 adapter for the Alloy framework.
//!
//! This module provides the main adapter that bridges OneBot v11 implementations
//! with the Alloy event system.

use std::sync::Arc;

use futures::future;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha1::Sha1;
use tracing::{trace, warn};

use crate::bot::OneBotBot;
use crate::config::{ConnectionConfig, OneBotConfig};
use crate::model::event::OneBotEvent;
use alloy_core::{
    Adapter, AdapterResult, Bot, BoxedEvent, ConnectionHandle, ConnectionHandler, ConnectionInfo,
    HttpClientConfig, HttpServerConfig, TransportContext, WsClientConfig, WsServerConfig,
};

/// The OneBot v11 adapter.
///
/// Supports multiple simultaneous connections of different types.
#[derive(Default)]
pub struct OneBotAdapter {
    /// Adapter configuration.
    config: OneBotConfig,
}

impl Adapter for OneBotAdapter {
    const NAME: &'static str = "onebot";

    type Config = OneBotConfig;

    type Bot = OneBotBot;

    fn from_config(config: Self::Config) -> Self {
        Self { config }
    }

    fn create_bot(&self, bot_id: &str, connection: &ConnectionHandle) -> Self::Bot {
        OneBotBot::new(bot_id, connection)
    }

    async fn on_message(&self, bot: &Self::Bot, data: &[u8]) -> Option<BoxedEvent> {
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
            bot.handle_response(&value);
            trace!(bot_id = %bot_id, echo = ?value.get("echo"), "Handled API response");
            return None; // API responses are not events
        }

        // Parse as event
        match serde_json::from_str::<OneBotEvent>(raw) {
            Ok(e) => Some(Arc::new(e)),
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, raw_data = %raw, "Failed to parse event raw data");
                None
            }
        }
    }

    async fn on_start(
        &self,
        transport: TransportContext,
        handler: Arc<dyn ConnectionHandler>,
    ) -> AdapterResult<()> {
        if self.config.connections.is_empty() {
            warn!("No connections in OneBot adapter configuration");
            return Ok(());
        }

        future::join_all(
            self.config
                .connections
                .iter()
                .map(|config| setup_connection(config, &transport, &handler)),
        )
        .await;

        Ok(())
    }
}

async fn setup_connection(
    conn_config: &ConnectionConfig,
    transport: &TransportContext,
    handler: &Arc<dyn ConnectionHandler>,
) {
    macro_rules! try_start {
        ($capability:ident, $config:expr, $resolve_fn:expr) => {
            if let Some(cap_fn) = transport.$capability() {
                if let Err(e) = cap_fn($config, handler.clone(), $resolve_fn).await {
                    warn!(error = %e, capability = stringify!($capability), "Failed to start connection");
                }
            } else {
                warn!(capability = stringify!($capability), "Capability not available");
            }
        }
    }

    match conn_config {
        ConnectionConfig::WsServer(config) => try_start!(
            ws_server,
            WsServerConfig::new(&config.host, config.port, &config.path),
            {
                let access_token = config.access_token.clone();
                Arc::new(move |conn_info| {
                    if let Some(token) = &access_token
                        && !conn_info.check_authorization(token)
                    {
                        return None;
                    }
                    resolve_bot_id(conn_info)
                })
            }
        ),

        ConnectionConfig::WsClient(config) => try_start!(
            ws_client,
            {
                let mut client_config = WsClientConfig::new(&config.url);
                if let Some(t) = config.access_token.as_ref().filter(|t| !t.is_empty()) {
                    client_config = client_config.with_token(t);
                }
                client_config
            },
            config.bot_id.clone()
        ),

        ConnectionConfig::HttpServer(config) => try_start!(
            http_server,
            HttpServerConfig::new(&config.host, config.port, &config.path),
            if let Some(secret) = &config.secret {
                let Ok(mac) = Hmac::<Sha1>::new_from_slice(secret.as_bytes()) else {
                    warn!("Failed to create HMAC instance for HTTP server authentication");
                    return;
                };

                Arc::new(move |conn_info| {
                    let signature = conn_info.headers.get("x-signature");
                    let body = conn_info.body.as_deref().unwrap_or_default();

                    if let Some(sig_bytes) =
                        signature.and_then(|s| hex::decode(s.trim_start_matches("sha1=")).ok())
                    {
                        let mut mac_clone = mac.clone();
                        mac_clone.update(body);
                        if mac_clone.verify_slice(&sig_bytes).is_err() {
                            return None;
                        }
                    } else {
                        return None;
                    }

                    resolve_bot_id(conn_info)
                })
            } else {
                Arc::new(resolve_bot_id)
            }
        ),

        ConnectionConfig::HttpClient(config) => try_start!(
            http_client,
            {
                let mut client_config = HttpClientConfig::new(&config.api_url);
                if let Some(token) = config.access_token.as_ref().filter(|t| !t.is_empty()) {
                    client_config = client_config.with_token(token);
                }
                client_config
            },
            Arc::new(|post_json| Box::pin(async move {
                let resp = post_json(
                    "",
                    json!({
                        "action": "get_login_info",
                        "params": {}
                    }),
                )
                .await
                .ok()?;

                resp.get("data")?
                    .get("user_id")?
                    .as_i64()
                    .map(|n| n.to_string())
            }))
        ),
    }
}

fn resolve_bot_id(conn_info: ConnectionInfo) -> Option<String> {
    conn_info.headers.get("x-self-id").cloned()
}
