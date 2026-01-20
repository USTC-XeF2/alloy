//! Configuration types for OneBot adapter.
//!
//! This module defines the configuration schema that can be loaded from
//! the global `alloy.yaml` configuration file.
//!
//! # Example Configuration
//!
//! ```yaml
//! adapters:
//!   onebot:
//!     connections:
//!       # WebSocket client - connect to a OneBot implementation
//!       - name: primary
//!         enabled: true
//!         type: ws-client
//!         url: ws://127.0.0.1:6700/ws
//!         access_token: ${BOT_TOKEN:-}
//!
//!       # WebSocket server - listen for incoming connections
//!       - name: listener
//!         enabled: false
//!         type: ws-server
//!         host: 0.0.0.0
//!         port: 8080
//!         path: /onebot/v11/ws
//!
//!       # HTTP webhook (receive events)
//!       - name: webhook
//!         enabled: false
//!         type: http-server
//!         host: 0.0.0.0
//!         port: 9000
//!         path: /onebot/callback
//!
//!       # HTTP client (send API calls)
//!       - name: api-client
//!         enabled: false
//!         type: http-client
//!         api_url: http://127.0.0.1:5700
//!
//!     # Global settings for all connections
//!     heartbeat_interval_secs: 30
//! ```

use serde::{Deserialize, Serialize};

/// OneBot adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OneBotConfig {
    /// List of connection configurations.
    pub connections: Vec<ConnectionConfig>,

    /// Default access token (used for connections without explicit token).
    pub default_access_token: Option<String>,

    /// Whether to auto-reconnect client connections.
    #[serde(default)]
    pub auto_reconnect: bool,

    /// Heartbeat interval in seconds (0 to disable).
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
}

fn default_heartbeat_interval() -> u64 {
    30
}

impl OneBotConfig {
    /// Returns only the enabled connections.
    pub fn enabled_connections(&self) -> impl Iterator<Item = &ConnectionConfig> {
        self.connections.iter().filter(|c| c.is_enabled())
    }

    /// Returns the number of enabled connections.
    pub fn enabled_count(&self) -> usize {
        self.connections.iter().filter(|c| c.is_enabled()).count()
    }
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
    /// Returns the connection name.
    pub fn name(&self) -> &str {
        match self {
            ConnectionConfig::WsServer(c) => &c.name,
            ConnectionConfig::WsClient(c) => &c.name,
            ConnectionConfig::HttpServer(c) => &c.name,
            ConnectionConfig::HttpClient(c) => &c.name,
        }
    }

    /// Returns whether this connection is enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            ConnectionConfig::WsServer(c) => c.enabled,
            ConnectionConfig::WsClient(c) => c.enabled,
            ConnectionConfig::HttpServer(c) => c.enabled,
            ConnectionConfig::HttpClient(c) => c.enabled,
        }
    }

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
    /// Connection name for identification.
    pub name: String,

    /// Whether this connection is enabled.
    pub enabled: bool,

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
            name: "ws-server".to_string(),
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 8080,
            path: "/onebot/v11/ws".to_string(),
            access_token: None,
        }
    }
}

impl WsServerConfig {
    /// Returns the bind address string.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// WebSocket client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WsClientConfig {
    /// Connection name for identification.
    pub name: String,

    /// Whether this connection is enabled.
    pub enabled: bool,

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
            name: "ws-client".to_string(),
            enabled: true,
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
    /// Connection name for identification.
    pub name: String,

    /// Whether this connection is enabled.
    pub enabled: bool,

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
            name: "http-server".to_string(),
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 9000,
            path: "/onebot/callback".to_string(),
            secret: None,
        }
    }
}

impl HttpServerConfig {
    /// Returns the bind address string.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// HTTP client configuration (for API calls).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpClientConfig {
    /// Connection name for identification.
    pub name: String,

    /// Whether this connection is enabled.
    pub enabled: bool,

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
            name: "http-client".to_string(),
            enabled: true,
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
        let yaml = r#"
connections:
  - name: main-server
    enabled: true
    type: ws-server
    host: 0.0.0.0
    port: 8080
    path: /ws
  - name: backup-client
    enabled: false
    type: ws-client
    url: ws://localhost:6700/ws
    access_token: secret
    auto_reconnect: true
heartbeat_interval_secs: 60
"#;

        let config: OneBotConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.connections.len(), 2);
        assert_eq!(config.heartbeat_interval_secs, 60);

        // Only 1 enabled
        assert_eq!(config.enabled_count(), 1);

        match &config.connections[0] {
            ConnectionConfig::WsServer(ws) => {
                assert_eq!(ws.name, "main-server");
                assert!(ws.enabled);
                assert_eq!(ws.port, 8080);
                assert_eq!(ws.path, "/ws");
            }
            _ => panic!("Expected WsServer"),
        }

        match &config.connections[1] {
            ConnectionConfig::WsClient(ws) => {
                assert_eq!(ws.name, "backup-client");
                assert!(!ws.enabled);
                assert_eq!(ws.url, "ws://localhost:6700/ws");
                assert_eq!(ws.access_token, Some("secret".to_string()));
            }
            _ => panic!("Expected WsClient"),
        }
    }

    #[test]
    fn test_enabled_connections() {
        let config = OneBotConfig {
            connections: vec![
                ConnectionConfig::WsClient(WsClientConfig {
                    name: "enabled".to_string(),
                    enabled: true,
                    ..Default::default()
                }),
                ConnectionConfig::WsClient(WsClientConfig {
                    name: "disabled".to_string(),
                    enabled: false,
                    ..Default::default()
                }),
            ],
            default_access_token: None,
            auto_reconnect: false,
            heartbeat_interval_secs: 30,
        };

        let enabled: Vec<_> = config.enabled_connections().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name(), "enabled");
    }
}
