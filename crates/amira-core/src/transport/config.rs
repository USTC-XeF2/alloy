//! Configuration types for transport clients and servers.
//!
//! Each transport variant has a corresponding config struct:
//! [`WsServerConfig`], [`WsClientConfig`], [`HttpServerConfig`], [`HttpClientConfig`],
//! and [`SseClientConfig`]. These are passed to capability functions during adapter startup.

use std::time::Duration;

/// Configuration for WebSocket server listeners.
#[derive(Debug, Clone)]
pub struct WsServerConfig {
    /// Bind address.
    pub bind_addr: String,
    /// Listen port.
    pub port: u16,
    /// WebSocket path.
    pub path: String,
}

impl WsServerConfig {
    /// Creates a new WebSocket server config with the given bind address and port.
    pub fn new(bind_addr: impl Into<String>, port: u16, path: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            port,
            path: path.into(),
        }
    }
}

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
    pub initial_delay: Option<Duration>,
    /// Maximum delay between reconnection attempts.
    pub max_delay: Option<Duration>,
    /// Backoff multiplier.
    pub backoff_multiplier: Option<f64>,
    /// Optional access token for authentication.
    pub access_token: Option<String>,
    /// Heartbeat interval for WebSocket keep-alive.
    pub heartbeat_interval: Option<Duration>,
}

impl WsClientConfig {
    /// Creates a new WebSocket client config with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            auto_reconnect: true,
            max_retries: None,
            initial_delay: None,
            max_delay: None,
            backoff_multiplier: None,
            access_token: None,
            heartbeat_interval: None,
        }
    }

    pub fn no_reconnect(mut self) -> Self {
        self.auto_reconnect = false;
        self
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

/// Configuration for HTTP server listeners.
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    /// Bind address.
    pub bind_addr: String,
    /// Listen port.
    pub port: u16,
    /// HTTP path for the listener.
    pub path: String,
}

impl HttpServerConfig {
    /// Creates a new HTTP server config with the given bind address and port.
    pub fn new(bind_addr: impl Into<String>, port: u16, path: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            port,
            path: path.into(),
        }
    }
}

/// Configuration for HTTP client connections.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Base URL for API endpoints.
    pub base_url: String,
    /// Optional access token for authentication (used as Bearer token).
    pub access_token: Option<String>,
    /// Request timeout duration.
    pub timeout: Option<Duration>,
}

impl HttpClientConfig {
    /// Creates a new HTTP client config.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            access_token: None,
            timeout: None,
        }
    }

    /// Sets the access token (used as Bearer token in Authorization header).
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Sets the request timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Configuration for SSE (Server-Sent Events) client connections.
#[derive(Debug, Clone)]
pub struct SseClientConfig {
    /// Full URL of the SSE endpoint.
    pub url: String,
    /// Optional access token for authentication (used as Bearer token).
    pub access_token: Option<String>,
    /// Whether to automatically reconnect on disconnect.
    pub auto_reconnect: bool,
    /// Initial delay between reconnection attempts.
    pub initial_delay: Option<Duration>,
    /// Maximum delay between reconnection attempts.
    pub max_delay: Option<Duration>,
    /// Maximum number of reconnection attempts (None = infinite).
    pub max_retries: Option<u32>,
    /// Backoff multiplier.
    pub backoff_multiplier: Option<f64>,
}

impl SseClientConfig {
    /// Creates a new SSE client config.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            access_token: None,
            auto_reconnect: true,
            initial_delay: None,
            max_delay: None,
            max_retries: None,
            backoff_multiplier: None,
        }
    }

    /// Sets the access token (used as Bearer token in Authorization header).
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Disables automatic reconnection.
    pub fn no_reconnect(mut self) -> Self {
        self.auto_reconnect = false;
        self
    }

    /// Sets the maximum retry count.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = Some(max);
        self
    }
}
