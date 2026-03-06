//! HTTP client capability implementation.

use std::sync::Arc;

use futures::FutureExt;
use reqwest::ClientBuilder;
use url::Url;

use alloy_core::{
    ConnectionHandler, HttpClientConfig, PostJsonFn, Sender, TransportError, TransportResult,
};
use alloy_macros::register_capability;

/// Registers an HTTP outbound API-client bot.
///
/// Builds a shared `reqwest` client, constructs a type-erased [`PostJsonFn`]
/// closure, and registers the bot via [`ConnectionHandler::register_connection`].
///
/// This function is registered as the `HttpStartClientFn` capability.
#[register_capability(http_client)]
pub async fn http_start_client(
    config: HttpClientConfig,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<()> {
    let mut builder = ClientBuilder::new();
    if let Some(timeout) = config.timeout {
        builder = builder.timeout(timeout);
    }
    let client = builder
        .build()
        .map_err(|e| TransportError::Io(e.to_string()))?;

    let base_url = Url::parse(&config.base_url)
        .map_err(|e| TransportError::Io(format!("Invalid base URL: {}", e)))?;

    let post_json: PostJsonFn = Arc::new(move |endpoint: &str, body| {
        let client = client.clone();
        let url = base_url.join(endpoint);
        let token = config.access_token.clone();
        async move {
            let url = url.map_err(|e| TransportError::Io(format!("Invalid URL: {}", e)))?;
            let mut req = client.post(url).json(&body);
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

    handler.register_connection(&config.bot_id, Some(Sender::HttpClient { post_json }));

    Ok(())
}
