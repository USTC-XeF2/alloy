//! Milky protocol adapter for the Amira framework.
//!
//! This module provides the main adapter that bridges Milky protocol v1.1
//! implementations with the Amira event system.
//!
//! # Connection Setup
//!
//! In the Milky protocol, the **protocol server** always provides two HTTP
//! endpoints.  Depending on the configured connection type, the adapter sets
//! up the corresponding transport pair:
//!
//! | Type            | Event Source | API calls        |
//! |-----------------|--------------|-----------------|
//! | `client`        | `none`       | HTTP `/api/*`   |
//! | `client`        | `sse`        | HTTP `/api/*`   |
//! | `client`        | `ws`         | HTTP `/api/*`   |
//! | `webhook`       | —            | — (receive only) |
//!
//! For `client` mode, the adapter always registers an HTTP client first and
//! then optionally creates an event intake connection based on `event_source`.

use std::sync::Arc;

use amira_core::adapter::Adapter;
use amira_core::error::AdapterResult;
use amira_core::transport::{
    ConnectionHandler, HttpClientConfig, HttpRequestFn, HttpServerConfig, Sender, SseClientConfig,
    TransportContext, WsClientConfig,
};
use amira_core::{Bot, Bytes, HttpMethod};
use futures::future;
use tracing::warn;

use crate::bot::MilkyBot;
use crate::config::{ConnectionConfig, EventSource, MilkyConfig};
use crate::model::api::{ApiResponse, LoginInfo};
use crate::model::event::MilkyEvent;

/// The Milky protocol adapter.
#[derive(Default)]
pub struct MilkyAdapter {
    config: MilkyConfig,
}

impl Adapter for MilkyAdapter {
    const NAME: &'static str = "milky";

    type Config = MilkyConfig;

    type Bot = MilkyBot;
    type Event = MilkyEvent;

    fn from_config(config: Self::Config) -> Self {
        Self { config }
    }

    fn create_bot(&self, bot_id: &str, sender: Option<Sender>) -> Self::Bot {
        MilkyBot::new(bot_id, sender)
    }

    async fn on_message(&self, bot: &Self::Bot, data: Bytes) -> Option<Self::Event> {
        let bot_id = bot.id();

        match serde_json::from_slice::<MilkyEvent>(&data) {
            Ok(e) => Some(e),
            Err(e) => {
                warn!(
                    bot_id = %bot_id,
                    error = %e,
                    "Failed to parse Milky event"
                );
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
            warn!("No connections in Milky adapter configuration");
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
        // ── Unified client ─────────────────────────────────────────
        // 1. Register HTTP client for /api/* calls.
        // 2. Optionally subscribe to /event by event_source.
        ConnectionConfig::Client {
            url,
            access_token,
            event_source,
        } => {
            let Some(http_client) = transport.http_client() else {
                warn!("HTTP client capability not available, skipping client connection");
                return;
            };

            let access_token = access_token.clone().filter(|t| !t.is_empty());

            let mut http_cfg = HttpClientConfig::new(url);
            if let Some(t) = &access_token {
                http_cfg = http_cfg.with_token(t);
            }

            let bot_id = match http_client(
                http_cfg,
                handler.clone(),
                Arc::new(|post_json| Box::pin(get_client_bot_id(post_json))),
            )
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    warn!(error = %e, "Failed to start HTTP client connection");
                    return;
                }
            };

            match event_source {
                EventSource::None => {}
                EventSource::Sse(reconnect) => try_start!(
                    sse_client,
                    {
                        let mut sse_cfg = SseClientConfig::new(format!("{url}/event"));
                        if let Some(t) = &access_token {
                            sse_cfg = sse_cfg.with_token(t);
                        }
                        sse_cfg.auto_reconnect = reconnect.auto_reconnect;
                        sse_cfg.initial_delay = Some(std::time::Duration::from_millis(
                            reconnect.reconnect_delay_ms,
                        ));
                        sse_cfg
                    },
                    bot_id
                ),
                EventSource::Ws(reconnect) => try_start!(
                    ws_client,
                    {
                        let mut ws_cfg = WsClientConfig::new(get_event_ws_url(url));
                        if let Some(t) = &access_token {
                            ws_cfg = ws_cfg.with_token(t);
                        }
                        ws_cfg.auto_reconnect = reconnect.auto_reconnect;
                        ws_cfg.initial_delay = Some(std::time::Duration::from_millis(
                            reconnect.reconnect_delay_ms,
                        ));
                        ws_cfg
                    },
                    bot_id
                ),
            }
        }

        // ── HTTP server (webhook) ───────────────────────────────────
        // Receive-only: Milky server pushes events via HTTP POST.
        ConnectionConfig::Webhook {
            host,
            port,
            path,
            access_token,
        } => try_start!(http_server, HttpServerConfig::new(host, *port, path), {
            let access_token = access_token.clone();
            Arc::new(move |conn_info| {
                // 1. Verify access token (Bearer)
                if let Some(token) = &access_token
                    && !conn_info.check_authorization(token)
                {
                    return None;
                }

                // 2. Extract bot_id from body self_id
                let body = conn_info.body.as_deref()?;
                let value: serde_json::Value = serde_json::from_slice(body).ok()?;
                value.get("self_id").and_then(|v| {
                    if v.is_string() {
                        v.as_str().map(std::string::ToString::to_string)
                    } else {
                        v.as_i64().map(|n| n.to_string())
                    }
                })
            })
        }),
    }
}

fn get_event_ws_url(url: &str) -> String {
    let scheme = if url.starts_with("https://") {
        "wss"
    } else {
        "ws"
    };
    let base = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    format!("{scheme}://{base}/event")
}

async fn get_client_bot_id(http_request: HttpRequestFn) -> Option<String> {
    match http_request(HttpMethod::POST, "/api/get_login_info", "{}".into()).await {
        Ok(data) => match serde_json::from_slice::<ApiResponse<LoginInfo>>(&data) {
            Ok(ApiResponse::Ok { data }) => Some(data.uin.to_string()),
            Ok(ApiResponse::Failed { retcode, message }) => {
                warn!(retcode, error = %message, "get_login_info API call failed");
                None
            }
            Err(e) => {
                warn!(error = %e, "Failed to parse get_login_info response");
                None
            }
        },
        Err(e) => {
            warn!(error = %e, "Failed to fetch login info");
            None
        }
    }
}
