//! HTTP client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tracing::{info, trace, warn};

use alloy_core::{
    ConnectionHandle, ConnectionHandler, HttpClientCapability, TransportError, TransportResult,
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
    async fn post_json(&self, url: &str, body: Value) -> TransportResult<Value> {
        trace!(url = %url, "HTTP POST JSON");

        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TransportError::Io(format!(
                "HTTP {} error: {}",
                status.as_u16(),
                text
            )));
        }

        let result = response
            .json()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(result)
    }

    async fn get(&self, url: &str) -> TransportResult<Value> {
        trace!(url = %url, "HTTP GET");

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TransportError::Io(format!(
                "HTTP {} error: {}",
                status.as_u16(),
                text
            )));
        }

        let result = response
            .json()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(result)
    }

    async fn start_client(
        &self,
        bot_id: &str,
        api_url: &str,
        access_token: Option<String>,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ConnectionHandle> {
        let bot_id = bot_id.to_string();
        let api_url = api_url.to_string();

        info!(bot_id = %bot_id, url = %api_url, "Starting HTTP client bot");

        // Create channel for outgoing API calls
        let (message_tx, mut message_rx) = mpsc::channel::<Vec<u8>>(256);
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        // Clone client for background task
        let client = self.client.clone();
        let bot_id_clone = bot_id.clone();

        // Spawn background task to send HTTP requests
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!(bot_id = %bot_id_clone, "HTTP client bot shutting down");
                            break;
                        }
                    }
                    Some(data) = message_rx.recv() => {
                        // Parse message as JSON and send via HTTP POST
                        match serde_json::from_slice::<Value>(&data) {
                            Ok(mut json) => {
                                // Add access_token if configured
                                if let Some(token) = &access_token {
                                    json["access_token"] = Value::String(token.clone());
                                }
                                // Send API request
                                match client.post(&api_url).json(&json).send().await {
                                    Ok(response) => {
                                        if !response.status().is_success() {
                                            warn!(
                                                bot_id = %bot_id_clone,
                                                status = %response.status(),
                                                "HTTP API request failed"
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            bot_id = %bot_id_clone,
                                            error = %e,
                                            "Failed to send HTTP API request"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    bot_id = %bot_id_clone,
                                    error = %e,
                                    "Failed to parse outgoing message as JSON"
                                );
                            }
                        }
                    }
                }
            }
        });

        // Create ConnectionHandle
        let connection = ConnectionHandle::new(bot_id.clone(), message_tx, shutdown_tx);

        // Create and register the bot
        handler.create_bot(&bot_id, connection.clone()).await;

        info!(bot_id = %bot_id, "HTTP client bot created successfully");

        Ok(connection)
    }
}
