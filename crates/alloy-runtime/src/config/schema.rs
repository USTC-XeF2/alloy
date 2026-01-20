//! Configuration schema definitions.

use alloy_core::TransportType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlloyConfig {
    /// Global settings that apply to all bots.
    #[serde(default)]
    pub global: GlobalConfig,

    /// Individual bot configurations.
    #[serde(default)]
    pub bots: Vec<BotConfig>,
}

/// Global configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Default timeout for network operations in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Default retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Enable metrics collection.
    #[serde(default)]
    pub enable_metrics: bool,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            timeout_ms: default_timeout_ms(),
            retry: RetryConfig::default(),
            enable_metrics: false,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_timeout_ms() -> u64 {
    30000
}

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Initial delay between retries in milliseconds.
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,

    /// Maximum delay between retries in milliseconds.
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,

    /// Exponential backoff multiplier.
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            backoff_multiplier: default_backoff_multiplier(),
        }
    }
}

impl RetryConfig {
    /// Converts to core retry config.
    pub fn to_core_retry(&self) -> alloy_core::RetryConfig {
        alloy_core::RetryConfig {
            max_retries: self.max_retries,
            initial_delay: Duration::from_millis(self.initial_delay_ms),
            max_delay: Duration::from_millis(self.max_delay_ms),
            multiplier: self.backoff_multiplier,
        }
    }
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay_ms() -> u64 {
    1000
}

fn default_max_delay_ms() -> u64 {
    30000
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

/// Individual bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Unique identifier for this bot instance.
    pub id: String,

    /// Human-readable name for this bot.
    #[serde(default)]
    pub name: Option<String>,

    /// Adapter type (e.g., "onebot").
    pub adapter: String,

    /// Transport configuration.
    pub transport: TransportConfig,

    /// Whether this bot is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Bot-specific settings.
    #[serde(default)]
    pub settings: HashMap<String, serde_yaml::Value>,
}

fn default_enabled() -> bool {
    true
}

/// Transport configuration.
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
    /// Returns the transport type.
    pub fn transport_type(&self) -> TransportType {
        match self {
            Self::WsClient(_) => TransportType::WsClient,
            Self::WsServer(_) => TransportType::WsServer,
            Self::HttpClient(_) => TransportType::HttpClient,
            Self::HttpServer(_) => TransportType::HttpServer,
        }
    }

    /// Converts runtime config to core config.
    pub fn to_core_config(&self) -> alloy_core::TransportConfig {
        match self {
            Self::WsClient(cfg) => {
                alloy_core::TransportConfig::WsClient(alloy_core::WsClientConfig {
                    url: cfg.url.clone(),
                    access_token: cfg.access_token.clone(),
                    auto_reconnect: cfg.auto_reconnect,
                    heartbeat_interval: Duration::from_secs(cfg.heartbeat_interval_secs),
                    retry: cfg.retry.as_ref().map(|r| r.to_core_retry()),
                })
            }
            Self::WsServer(cfg) => {
                alloy_core::TransportConfig::WsServer(alloy_core::WsServerConfig {
                    host: cfg.host.clone(),
                    port: cfg.port,
                    path: cfg.path.clone(),
                    access_token: cfg.access_token.clone(),
                })
            }
            Self::HttpClient(cfg) => {
                alloy_core::TransportConfig::HttpClient(alloy_core::HttpClientConfig {
                    url: cfg.url.clone(),
                    access_token: cfg.access_token.clone(),
                    timeout: Duration::from_millis(cfg.timeout_ms),
                    retry: cfg.retry.as_ref().map(|r| r.to_core_retry()),
                })
            }
            Self::HttpServer(cfg) => {
                alloy_core::TransportConfig::HttpServer(alloy_core::HttpServerConfig {
                    host: cfg.host.clone(),
                    port: cfg.port,
                    path: cfg.path.clone(),
                    secret: cfg.secret.clone(),
                })
            }
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
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,

    /// Heartbeat interval in seconds.
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Retry configuration override.
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}

fn default_auto_reconnect() -> bool {
    true
}

fn default_heartbeat_interval() -> u64 {
    30
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

/// HTTP client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpClientConfig {
    /// HTTP server URL to connect to.
    pub url: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Request timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Retry configuration override.
    #[serde(default)]
    pub retry: Option<RetryConfig>,
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
