//! HTTP client capability implementation.

use std::sync::Arc;

use amira_core::error::{TransportError, TransportResult};
use amira_core::transport::{
    ClientBotIdFn, ConnectionHandler, HttpClientConfig, HttpRequestFn, Sender,
};
use amira_macros::register_capability;
use futures::FutureExt;
use reqwest::ClientBuilder;
use reqwest::header::CONTENT_TYPE;
use url::Url;

/// Registers an HTTP outbound API-client bot.
///
/// Builds a shared `reqwest` client, constructs a type-erased [`HttpRequestFn`]
/// closure, and registers the bot via [`ConnectionHandler::register_connection`].
///
/// This function is registered as the `HttpStartClientFn` capability.
#[register_capability(http_client)]
pub async fn http_start_client(
    config: HttpClientConfig,
    handler: Arc<dyn ConnectionHandler>,
    resolve_bot_id: ClientBotIdFn,
) -> TransportResult<String> {
    let mut builder = ClientBuilder::new();
    if let Some(timeout) = config.timeout {
        builder = builder.timeout(timeout);
    }
    let client = builder
        .build()
        .map_err(|e| TransportError::Io(e.to_string()))?;

    let base_url =
        Url::parse(&config.base_url).map_err(|e| TransportError::Serialization(e.to_string()))?;

    let http_request: HttpRequestFn = Arc::new(move |method, endpoint, body| {
        let client = client.clone();
        let url = base_url.join(endpoint);
        let token = config.access_token.clone();
        async move {
            let url = url.map_err(|e| TransportError::Serialization(e.to_string()))?;
            let mut req = client
                .request(method, url)
                .header(CONTENT_TYPE, "application/json")
                .body(body);
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
            resp.bytes()
                .await
                .map_err(|e| TransportError::Io(e.to_string()))
        }
        .boxed()
    });

    let Some(bot_id) = resolve_bot_id(http_request.clone()).await else {
        return Err(TransportError::ConnectionFailed {
            url: config.base_url.clone(),
            reason: "Failed to resolve bot ID from HTTP client http_request callback".to_string(),
        });
    };

    handler.register_connection(&bot_id, Some(Sender::HttpClient { http_request }));

    Ok(bot_id)
}
