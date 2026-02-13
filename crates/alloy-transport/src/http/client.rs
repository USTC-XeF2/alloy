//! HTTP client capability implementation.

use std::time::Duration;

use alloy_core::{HttpClientCapability, TransportError, TransportResult};
use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use tracing::trace;

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
    async fn post_json(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> TransportResult<serde_json::Value> {
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

    async fn get(&self, url: &str) -> TransportResult<serde_json::Value> {
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
}
