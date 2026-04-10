//! Configuration types for OneBot adapter.
//!
//! This module defines the configuration schema that can be loaded from
//! the global `alloy.toml` configuration file.

use serde::Deserialize;

/// OneBot adapter configuration.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct OneBotConfig {
    /// List of connection configurations.
    pub connections: Vec<ConnectionConfig>,
}

/// Connection configuration for a single connection.
///
/// Uses tagged union with `type` field to determine the variant.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ConnectionConfig {
    /// WebSocket server - listens for incoming connections.
    WsServer(WsServerConfig),

    /// WebSocket client - connects to a OneBot implementation.
    WsClient(WsClientConfig),

    /// HTTP server - receives webhook callbacks.
    HttpServer(HttpServerConfig),

    /// HTTP client - sends API requests via HTTP.
    HttpClient(HttpClientConfig),
}

impl ConnectionConfig {
    /// Returns the access token if configured.
    pub fn access_token(&self) -> Option<&str> {
        match self {
            ConnectionConfig::WsServer(c) => c.access_token.as_deref(),
            ConnectionConfig::WsClient(c) => c.access_token.as_deref(),
            ConnectionConfig::HttpServer(c) => c.secret.as_deref(),
            ConnectionConfig::HttpClient(c) => c.access_token.as_deref(),
        }
    }
}

/// WebSocket server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WsServerConfig {
    /// Bind address (default: "127.0.0.1").
    #[serde(default = "default_host")]
    pub host: String,

    /// Listen port.
    pub port: u16,

    /// WebSocket path (default: "/onebot/v11/ws").
    #[serde(default = "default_ws_path")]
    pub path: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_ws_path() -> String {
    "/onebot/v11/ws".into()
}

/// WebSocket client configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WsClientConfig {
    /// WebSocket URL to connect to.
    pub url: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Whether to automatically reconnect on disconnection.
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,

    /// Reconnection delay in milliseconds.
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
}

const fn default_auto_reconnect() -> bool {
    true
}

const fn default_reconnect_delay_ms() -> u64 {
    5000
}

/// HTTP server configuration (for webhooks).
#[derive(Debug, Clone, Deserialize)]
pub struct HttpServerConfig {
    /// Bind address (default: "127.0.0.1").
    #[serde(default = "default_host")]
    pub host: String,

    /// Listen port.
    pub port: u16,

    /// Webhook path (default: "/onebot/v11").
    #[serde(default = "default_webhook_path")]
    pub path: String,

    /// Secret for verifying webhook signatures.
    #[serde(default)]
    pub secret: Option<String>,
}

fn default_webhook_path() -> String {
    "/onebot/v11".into()
}

/// HTTP client configuration (for API calls).
#[derive(Debug, Clone, Deserialize)]
pub struct HttpClientConfig {
    /// HTTP API URL.
    pub api_url: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Request timeout in milliseconds (default: 30000).
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_timeout_ms() -> u64 {
    30000
}
