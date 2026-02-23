//! HTTP client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use alloy_macros::register_capability;
use futures::FutureExt;
use reqwest::ClientBuilder;
use tokio::sync::watch;
use tracing::info;

use alloy_core::{
    ConnectionHandle, ConnectionHandler, PostJsonFn, TransportError, TransportResult,
};

/// Registers an HTTP outbound API-client bot.
///
/// The returned [`ConnectionHandle`] lets the bot send JSON API calls; it does
/// not receive events through this connection.
///
/// This function is registered as the `HttpStartClientFn` capability.
#[register_capability(http_client)]
pub async fn http_start_client(
    bot_id: String,
    api_url: String,
    access_token: Option<String>,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<ConnectionHandle> {
    info!(bot_id = %bot_id, url = %api_url, "Registering HTTP API client bot");

    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| TransportError::Io(e.to_string()))?;
    let post_json: PostJsonFn = Arc::new(move |body| {
        let client = client.clone();
        let url = api_url.clone();
        let token = access_token.clone();
        async move {
            let mut req = client.post(&url).json(&body);
            if let Some(t) = &token {
                req = req.bearer_auth(t);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(TransportError::Io(format!(
                    "HTTP {} error: {}",
                    status.as_u16(),
                    text
                )));
            }
            resp.json()
                .await
                .map_err(|e| TransportError::Io(e.to_string()))
        }
        .boxed()
    });

    let (shutdown_tx, _shutdown_rx) = watch::channel(false);
    let connection = ConnectionHandle::new_http_client(&bot_id, post_json, shutdown_tx);

    handler.create_bot(&bot_id, connection.clone()).await;

    info!(bot_id = %bot_id, "HTTP API client bot registered");
    Ok(connection)
}
