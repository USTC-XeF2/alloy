//! Transport type definitions and configuration abstractions.
//!
//! This module defines the transport types and their configurations
//! at an abstract level. The actual implementations are in `alloy-transport`.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Supported transport types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportType {
    /// WebSocket client - connects to a WebSocket server.
    WsClient,
    /// WebSocket server - accepts WebSocket connections.
    WsServer,
    /// HTTP client - makes HTTP requests.
    HttpClient,
    /// HTTP server - receives HTTP callbacks.
    HttpServer,
}

impl TransportType {
    /// Returns true if this is a client transport type.
    pub fn is_client(&self) -> bool {
        matches!(self, Self::WsClient | Self::HttpClient)
    }

    /// Returns true if this is a server transport type.
    pub fn is_server(&self) -> bool {
        matches!(self, Self::WsServer | Self::HttpServer)
    }
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WsClient => write!(f, "ws-client"),
            Self::WsServer => write!(f, "ws-server"),
            Self::HttpClient => write!(f, "http-client"),
            Self::HttpServer => write!(f, "http-server"),
        }
    }
}

/// Retry configuration for transport connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial delay between retries.
    #[serde(with = "humantime_serde", default = "default_initial_delay")]
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    #[serde(with = "humantime_serde", default = "default_max_delay")]
    pub max_delay: Duration,
    /// Multiplier for exponential backoff.
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay() -> Duration {
    Duration::from_secs(1)
}

fn default_max_delay() -> Duration {
    Duration::from_secs(30)
}

fn default_multiplier() -> f64 {
    2.0
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay: default_initial_delay(),
            max_delay: default_max_delay(),
            multiplier: default_multiplier(),
        }
    }
}

/// WebSocket client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClientConfig {
    /// WebSocket server URL to connect to.
    pub url: String,
    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,
    /// Auto-reconnect on disconnection.
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    /// Heartbeat interval.
    #[serde(with = "humantime_serde", default = "default_heartbeat")]
    pub heartbeat_interval: Duration,
    /// Retry configuration (uses default if None).
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}

fn default_true() -> bool {
    true
}

fn default_heartbeat() -> Duration {
    Duration::from_secs(30)
}

impl WsClientConfig {
    /// Creates a new WebSocket client configuration.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            access_token: None,
            auto_reconnect: true,
            heartbeat_interval: default_heartbeat(),
            retry: None,
        }
    }

    /// Returns the retry config, using default if not specified.
    pub fn retry_config(&self) -> RetryConfig {
        self.retry.clone().unwrap_or_default()
    }
}

/// WebSocket server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsServerConfig {
    /// Host address to bind to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    pub port: u16,
    /// Path for WebSocket endpoint.
    #[serde(default = "default_ws_path")]
    pub path: String,
    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_ws_path() -> String {
    "/ws".to_string()
}

impl WsServerConfig {
    /// Creates a new WebSocket server configuration.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            path: default_ws_path(),
            access_token: None,
        }
    }
}

/// HTTP client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpClientConfig {
    /// HTTP server URL.
    pub url: String,
    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,
    /// Request timeout.
    #[serde(with = "humantime_serde", default = "default_timeout")]
    pub timeout: Duration,
    /// Retry configuration (uses default if None).
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

impl HttpClientConfig {
    /// Creates a new HTTP client configuration.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            access_token: None,
            timeout: default_timeout(),
            retry: None,
        }
    }

    /// Returns the retry config, using default if not specified.
    pub fn retry_config(&self) -> RetryConfig {
        self.retry.clone().unwrap_or_default()
    }
}

/// HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Host address to bind to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    pub port: u16,
    /// Path for callback endpoint.
    #[serde(default = "default_http_path")]
    pub path: String,
    /// Secret for signature verification.
    #[serde(default)]
    pub secret: Option<String>,
}

fn default_http_path() -> String {
    "/".to_string()
}

impl HttpServerConfig {
    /// Creates a new HTTP server configuration.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            path: default_http_path(),
            secret: None,
        }
    }
}

/// Transport configuration enum.
///
/// This represents the configuration for any supported transport type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransportConfig {
    /// WebSocket client configuration.
    WsClient(WsClientConfig),
    /// WebSocket server configuration.
    WsServer(WsServerConfig),
    /// HTTP client configuration.
    HttpClient(HttpClientConfig),
    /// HTTP server configuration.
    HttpServer(HttpServerConfig),
}

impl TransportConfig {
    /// Returns the transport type for this configuration.
    pub fn transport_type(&self) -> TransportType {
        match self {
            Self::WsClient(_) => TransportType::WsClient,
            Self::WsServer(_) => TransportType::WsServer,
            Self::HttpClient(_) => TransportType::HttpClient,
            Self::HttpServer(_) => TransportType::HttpServer,
        }
    }

    /// Gets the WebSocket client configuration if this is a WsClient.
    pub fn as_ws_client(&self) -> Option<&WsClientConfig> {
        match self {
            Self::WsClient(c) => Some(c),
            _ => None,
        }
    }

    /// Gets the WebSocket server configuration if this is a WsServer.
    pub fn as_ws_server(&self) -> Option<&WsServerConfig> {
        match self {
            Self::WsServer(c) => Some(c),
            _ => None,
        }
    }

    /// Gets the HTTP client configuration if this is a HttpClient.
    pub fn as_http_client(&self) -> Option<&HttpClientConfig> {
        match self {
            Self::HttpClient(c) => Some(c),
            _ => None,
        }
    }

    /// Gets the HTTP server configuration if this is a HttpServer.
    pub fn as_http_server(&self) -> Option<&HttpServerConfig> {
        match self {
            Self::HttpServer(c) => Some(c),
            _ => None,
        }
    }
}

/// Serde module for humantime Duration serialization.
mod humantime_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}s", duration.as_secs()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if let Some(secs) = s.strip_suffix("s") {
            secs.trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|e| e.to_string())
        } else if let Some(ms) = s.strip_suffix("ms") {
            ms.trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|e| e.to_string())
        } else if let Some(mins) = s.strip_suffix("m") {
            mins.trim()
                .parse::<u64>()
                .map(|m| Duration::from_secs(m * 60))
                .map_err(|e| e.to_string())
        } else {
            // Default to seconds
            s.parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|e| e.to_string())
        }
    }
}
