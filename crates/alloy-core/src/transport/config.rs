//! Configuration types for transport clients.

use std::time::Duration;

// =============================================================================
// WebSocket Client Config
// =============================================================================

/// Configuration for WebSocket client connections.
#[derive(Debug, Clone)]
pub struct WsClientConfig {
    /// WebSocket server URL.
    pub url: String,
    /// Whether to automatically reconnect on disconnect.
    pub auto_reconnect: bool,
    /// Maximum number of reconnection attempts (None = infinite).
    pub max_retries: Option<u32>,
    /// Initial delay between reconnection attempts.
    pub initial_delay: Duration,
    /// Maximum delay between reconnection attempts.
    pub max_delay: Duration,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
    /// Optional access token for authentication.
    pub access_token: Option<String>,
    /// Heartbeat interval for WebSocket keep-alive.
    pub heartbeat_interval: Option<Duration>,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            auto_reconnect: true,
            max_retries: None, // Infinite retries
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            access_token: None,
            heartbeat_interval: Some(Duration::from_secs(30)),
        }
    }
}

impl WsClientConfig {
    /// Creates a new WebSocket client config with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Creates a new WebSocket client config with auto-reconnect disabled.
    pub fn no_reconnect(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            auto_reconnect: false,
            ..Default::default()
        }
    }

    /// Sets the access token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Sets the maximum retry count.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = Some(max);
        self
    }
}

// =============================================================================
// HTTP Client Config
// =============================================================================

/// Configuration for HTTP client connections.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// API endpoint URL.
    pub api_url: String,
    /// Optional access token for authentication (used as Bearer token).
    pub access_token: Option<String>,
    /// Request timeout duration.
    pub timeout: Duration,
}

impl HttpClientConfig {
    /// Creates a new HTTP client config with the given API URL.
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into(),
            access_token: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Sets the access token (used as Bearer token in Authorization header).
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Sets the request timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self::new("")
    }
}
