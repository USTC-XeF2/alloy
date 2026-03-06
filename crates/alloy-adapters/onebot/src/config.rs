//! Configuration types for OneBot adapter.
//!
//! This module defines the configuration schema that can be loaded from
//! the global `alloy.toml` configuration file.
//!
//! # Example Configuration
//!
//! ```toml
//! # WebSocket client - connect to a OneBot implementation
//! [[adapters.onebot.connections]]
//! name = "primary"
//! enabled = true
//! type = "ws-client"
//! url = "ws://127.0.0.1:6700/ws"
//! access_token = "${BOT_TOKEN:-}"
//!
//! # WebSocket server - listen for incoming connections
//! [[adapters.onebot.connections]]
//! name = "listener"
//! enabled = false
//! type = "ws-server"
//! host = "0.0.0.0"
//! port = 8080
//! path = "/onebot/v11/ws"
//!
//! # HTTP webhook (receive events)
//! [[adapters.onebot.connections]]
//! name = "webhook"
//! enabled = false
//! type = "http-server"
//! host = "0.0.0.0"
//! port = 9000
//! path = "/onebot/callback"
//!
//! # HTTP client (send API calls)
//! [[adapters.onebot.connections]]
//! name = "api-client"
//! enabled = false
//! type = "http-client"
//! api_url = "http://127.0.0.1:5700"
//! ```

use serde::{Deserialize, Serialize};

/// OneBot adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OneBotConfig {
    /// List of connection configurations.
    pub connections: Vec<ConnectionConfig>,
}

/// Connection configuration for a single connection.
///
/// Uses tagged union with `type` field to determine the variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WsServerConfig {
    /// Bind address (default: "0.0.0.0").
    pub host: String,

    /// Listen port (default: 8080).
    pub port: u16,

    /// WebSocket path (default: "/onebot/v11/ws").
    pub path: String,

    /// Access token for authentication.
    pub access_token: Option<String>,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            path: "/onebot/v11/ws".to_string(),
            access_token: None,
        }
    }
}

/// WebSocket client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WsClientConfig {
    /// WebSocket URL to connect to.
    pub url: String,

    /// Access token for authentication.
    pub access_token: Option<String>,

    /// Whether to automatically reconnect on disconnection.
    pub auto_reconnect: bool,

    /// Reconnection delay in milliseconds.
    pub reconnect_delay_ms: u64,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:6700/ws".to_string(),
            access_token: None,
            auto_reconnect: true,
            reconnect_delay_ms: 5000,
        }
    }
}

/// HTTP server configuration (for webhooks).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpServerConfig {
    /// Bind address (default: "0.0.0.0").
    pub host: String,

    /// Listen port (default: 9000).
    pub port: u16,

    /// Webhook path (default: "/onebot/callback").
    pub path: String,

    /// Secret for verifying webhook signatures.
    pub secret: Option<String>,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9000,
            path: "/onebot/callback".to_string(),
            secret: None,
        }
    }
}

/// HTTP client configuration (for API calls).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpClientConfig {
    /// Bot ID for this HTTP client.
    /// Required since HTTP clients don't have incoming connections to extract ID from.
    pub bot_id: String,

    /// HTTP API URL.
    pub api_url: String,

    /// Access token for authentication.
    pub access_token: Option<String>,

    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            bot_id: "12345678".to_string(), // Default bot ID
            api_url: "http://127.0.0.1:5700".to_string(),
            access_token: None,
            timeout_ms: 30000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_config() {
        let config = OneBotConfig {
            connections: vec![
                ConnectionConfig::WsServer(WsServerConfig {
                    host: "0.0.0.0".to_string(),
                    port: 8080,
                    path: "/ws".to_string(),
                    access_token: None,
                }),
                ConnectionConfig::WsClient(WsClientConfig {
                    url: "ws://localhost:6700/ws".to_string(),
                    access_token: Some("secret".to_string()),
                    ..Default::default()
                }),
            ],
        };

        assert_eq!(config.connections.len(), 2);

        match &config.connections[0] {
            ConnectionConfig::WsServer(ws) => {
                assert_eq!(ws.port, 8080);
                assert_eq!(ws.path, "/ws");
            }
            _ => panic!("Expected WsServer"),
        }

        match &config.connections[1] {
            ConnectionConfig::WsClient(ws) => {
                assert_eq!(ws.url, "ws://localhost:6700/ws");
                assert_eq!(ws.access_token, Some("secret".to_string()));
            }
            _ => panic!("Expected WsClient"),
        }
    }
}
