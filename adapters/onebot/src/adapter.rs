//! OneBot v11 adapter for the Amira framework.
//!
//! This module provides the main adapter that bridges OneBot v11 implementations
//! with the Amira event system.

use std::sync::Arc;

use futures::future;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use tracing::{trace, warn};

use crate::bot::OneBotBot;
use crate::config::{ConnectionConfig, OneBotConfig};
use crate::model::event::OneBotEvent;
use amira_core::adapter::Adapter;
use amira_core::error::AdapterResult;
use amira_core::transport::{
    ConnectionHandler, ConnectionInfo, HttpClientConfig, HttpServerConfig, Sender,
    TransportContext, WsClientConfig, WsServerConfig,
};
use amira_core::{Bot, Bytes, HttpMethod};

/// The OneBot v11 adapter.
///
/// Supports multiple simultaneous connections of different types.
#[derive(Default)]
pub struct OneBotAdapter {
    config: OneBotConfig,
}

impl Adapter for OneBotAdapter {
    const NAME: &'static str = "onebot";

    type Config = OneBotConfig;

    type Bot = OneBotBot;
    type Event = OneBotEvent;

    fn from_config(config: Self::Config) -> Self {
        Self { config }
    }

    fn create_bot(&self, bot_id: &str, sender: Option<Sender>) -> Self::Bot {
        OneBotBot::new(bot_id, sender)
    }

    async fn on_message(&self, bot: &Self::Bot, data: Bytes) -> Option<Self::Event> {
        let bot_id = bot.id();

        // Try to handle as API response
        let Err(data) = bot.try_handle_response(data) else {
            trace!(bot_id = %bot_id, "Handled API response");
            return None;
        };

        // Parse as event
        match serde_json::from_slice::<OneBotEvent>(&data) {
            Ok(e) => Some(e),
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to parse event raw data");
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
                    let signature = conn_info.get_header("x-signature");
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
            Arc::new(|request| Box::pin(async move {
                let resp = request(
                    HttpMethod::POST,
                    "",
                    r#"{"action": "get_login_info"}"#.into(),
                )
                .await
                .ok()?;

                serde_json::from_slice::<serde_json::Value>(&resp)
                    .ok()?
                    .get("data")?
                    .get("user_id")?
                    .as_i64()
                    .map(|n| n.to_string())
            }))
        ),
    }
}

fn resolve_bot_id(conn_info: ConnectionInfo) -> Option<String> {
    conn_info.get_header("x-self-id").map(ToString::to_string)
}
