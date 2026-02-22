//! HTTP client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::FutureExt;
use reqwest::{Client, ClientBuilder};
use tokio::sync::watch;
use tracing::info;

use alloy_core::{
    ConnectionHandle, ConnectionHandler, HttpClientCapability, PostJsonFn, TransportError,
    TransportResult,
};

/// HTTP client capability implementation.
pub struct HttpClientCapabilityImpl {
    client: Client,
}

impl HttpClientCapabilityImpl {
    /// Creates a new HTTP client capability.
    pub fn new() -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Creates with custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = ClientBuilder::new()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }
}

impl Default for HttpClientCapabilityImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClientCapability for HttpClientCapabilityImpl {
    async fn start_client(
        &self,
        bot_id: &str,
        api_url: &str,
        access_token: Option<String>,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ConnectionHandle> {
        info!(bot_id = %bot_id, url = %api_url, "Registering HTTP API client bot");

        // Capture everything connection-specific inside the closure so that
        // ConnectionHandle carries only the behaviour primitive.
        let client = self.client.clone();
        let api_url_owned = api_url.to_string();
        let access_token_owned = access_token.clone();
        let post_json: PostJsonFn = Arc::new(move |body| {
            let client = client.clone();
            let url = api_url_owned.clone();
            let token = access_token_owned.clone();
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
        let connection = ConnectionHandle::new_http_client(bot_id, post_json, shutdown_tx);

        handler.create_bot(bot_id, connection.clone()).await;

        info!(bot_id = %bot_id, "HTTP API client bot registered");
        Ok(connection)
    }
}
