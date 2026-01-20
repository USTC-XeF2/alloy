//! Configuration schema definitions using figment.
//!
//! This module defines the configuration structure for the Alloy framework.
//! The design prioritizes:
//!
//! - **Extensibility**: Adapters can define their own configuration sections
//! - **Decoupling**: Core config is separate from adapter-specific config  
//! - **Multi-source**: Supports files, env vars, and programmatic config
//! - **Type safety**: Strong typing with serde and figment extraction
//!
//! # Configuration Hierarchy
//!
//! ```text
//! AlloyConfig
//! ├── logging: LoggingConfig       # Logging settings
//! ├── network: NetworkConfig       # Global network defaults
//! ├── runtime: RuntimeConfig       # Runtime behavior
//! └── adapters: Map<String, Value> # Adapter-specific configs (dynamic)
//! ```
//!
//! # Example Configuration (YAML)
//!
//! ```yaml
//! logging:
//!   level: debug
//!   format: pretty
//!   
//! network:
//!   timeout_secs: 30
//!   
//! adapters:
//!   onebot:
//!     connections:
//!       - type: ws-client
//!         url: ws://127.0.0.1:8080
//! ```

use figment::value::{Tag, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

// =============================================================================
// Root Configuration
// =============================================================================

/// Root configuration structure for the Alloy framework.
///
/// This struct is designed to be extended by adapters through the `adapters` field,
/// which holds adapter-specific configuration as dynamic values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AlloyConfig {
    /// Logging configuration.
    pub logging: LoggingConfig,

    /// Global network configuration (defaults for all connections).
    pub network: NetworkConfig,

    /// Runtime configuration.
    pub runtime: RuntimeConfig,

    /// Adapter-specific configurations.
    ///
    /// Each adapter registers its own configuration schema.
    /// Example: `adapters.onebot` contains OneBot-specific settings.
    #[serde(default)]
    pub adapters: HashMap<String, Value>,
}

impl AlloyConfig {
    /// Extracts adapter-specific configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let onebot_config: OneBotConfig = config.extract_adapter("onebot")?;
    /// ```
    pub fn extract_adapter<T: serde::de::DeserializeOwned>(
        &self,
        adapter_name: &str,
    ) -> Result<T, figment::Error> {
        let value = self
            .adapters
            .get(adapter_name)
            .cloned()
            .unwrap_or_else(|| Value::Dict(Tag::default(), BTreeMap::default()));

        figment::Figment::from(figment::providers::Serialized::defaults(value)).extract()
    }

    /// Checks if an adapter has configuration.
    pub fn has_adapter(&self, adapter_name: &str) -> bool {
        self.adapters.contains_key(adapter_name)
    }
}

// =============================================================================
// Logging Configuration
// =============================================================================

/// Logging configuration.
///
/// Supports multiple output formats, targets, and filtering options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Default log level.
    ///
    /// Can be: trace, debug, info, warn, error
    pub level: LogLevel,

    /// Output format.
    pub format: LogFormat,

    /// Output target.
    pub output: LogOutput,

    /// Whether to include timestamps.
    pub timestamps: bool,

    /// Whether to include source file location.
    pub file_location: bool,

    /// Whether to include thread IDs.
    pub thread_ids: bool,

    /// Whether to include thread names.
    pub thread_names: bool,

    /// Span event configuration for Tower Service visibility.
    pub span_events: SpanEventConfig,

    /// Module-specific log level overrides.
    ///
    /// Example: `{ "alloy_transport": "debug", "hyper": "warn" }`
    #[serde(default)]
    pub filters: HashMap<String, LogLevel>,

    /// Log file path (only used when output is "file").
    pub file_path: Option<PathBuf>,

    /// Maximum log file size in bytes before rotation (default: 10MB).
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Number of rotated log files to keep.
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Pretty,
            output: LogOutput::Stdout,
            timestamps: true,
            file_location: false,
            thread_ids: false,
            thread_names: false,
            span_events: SpanEventConfig::default(),
            filters: HashMap::new(),
            file_path: None,
            max_file_size: default_max_file_size(),
            max_files: default_max_files(),
        }
    }
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 // 10 MB
}

fn default_max_files() -> u32 {
    5
}

/// Log level enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Converts to tracing::Level.
    pub fn to_tracing_level(self) -> tracing::Level {
        match self {
            Self::Trace => tracing::Level::TRACE,
            Self::Debug => tracing::Level::DEBUG,
            Self::Info => tracing::Level::INFO,
            Self::Warn => tracing::Level::WARN,
            Self::Error => tracing::Level::ERROR,
        }
    }

    /// Converts to filter directive string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable pretty format.
    #[default]
    Pretty,
    /// Compact single-line format.
    Compact,
    /// JSON format for structured logging.
    Json,
    /// Full verbose format.
    Full,
}

/// Log output target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    /// Output to stdout.
    #[default]
    Stdout,
    /// Output to stderr.
    Stderr,
    /// Output to file (requires `file_path`).
    File,
}

/// Span event configuration for Tower Service observability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SpanEventConfig {
    /// Log when a span is created.
    pub new: bool,
    /// Log when a span is entered.
    pub enter: bool,
    /// Log when a span is exited.
    pub exit: bool,
    /// Log when a span is closed.
    pub close: bool,
}

impl SpanEventConfig {
    /// No span events.
    pub const NONE: Self = Self {
        new: false,
        enter: false,
        exit: false,
        close: false,
    };

    /// Lifecycle events (new + close).
    pub const LIFECYCLE: Self = Self {
        new: true,
        enter: false,
        exit: false,
        close: true,
    };

    /// All span events.
    pub const FULL: Self = Self {
        new: true,
        enter: true,
        exit: true,
        close: true,
    };
}

// =============================================================================
// Network Configuration
// =============================================================================

/// Global network configuration defaults.
///
/// These settings apply to all connections unless overridden at the connection level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    /// Default connection timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Default retry configuration.
    pub retry: RetryConfig,

    /// Whether to auto-reconnect by default.
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,

    /// Heartbeat interval in seconds (0 to disable).
    #[serde(default = "default_heartbeat_secs")]
    pub heartbeat_secs: u64,

    /// HTTP proxy URL (optional).
    pub http_proxy: Option<String>,

    /// HTTPS proxy URL (optional).
    pub https_proxy: Option<String>,

    /// Hosts to bypass proxy for.
    #[serde(default)]
    pub no_proxy: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout_secs(),
            retry: RetryConfig::default(),
            auto_reconnect: true,
            heartbeat_secs: default_heartbeat_secs(),
            http_proxy: None,
            https_proxy: None,
            no_proxy: Vec::new(),
        }
    }
}

impl NetworkConfig {
    /// Returns the timeout as Duration.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Returns the heartbeat interval as Duration.
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.heartbeat_secs)
    }
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_heartbeat_secs() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
    /// Converts to alloy_core::RetryConfig.
    pub fn to_core_config(&self) -> alloy_core::RetryConfig {
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

// =============================================================================
// Runtime Configuration
// =============================================================================

/// Runtime behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Graceful shutdown timeout in seconds.
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,

    /// Enable metrics collection.
    #[serde(default)]
    pub enable_metrics: bool,

    /// Metrics server port (only when metrics enabled).
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Event channel buffer size.
    #[serde(default = "default_event_buffer")]
    pub event_buffer_size: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_secs: default_shutdown_timeout(),
            enable_metrics: false,
            metrics_port: default_metrics_port(),
            event_buffer_size: default_event_buffer(),
        }
    }
}

impl RuntimeConfig {
    /// Returns shutdown timeout as Duration.
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_secs)
    }
}

fn default_shutdown_timeout() -> u64 {
    30
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_event_buffer() -> usize {
    1024
}

// =============================================================================
// Connection Configuration (for adapters to use)
// =============================================================================

/// Common connection configuration that adapters can use.
///
/// This is provided as a helper type for adapters to define their own
/// connection configurations with consistent structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ConnectionConfig {
    /// WebSocket client connection.
    WsClient(WsClientConfig),
    /// WebSocket server listener.
    WsServer(WsServerConfig),
    /// HTTP client (polling or webhooks).
    HttpClient(HttpClientConfig),
    /// HTTP server (receiving webhooks).
    HttpServer(HttpServerConfig),
}

/// WebSocket client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClientConfig {
    /// WebSocket server URL.
    pub url: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Override auto-reconnect setting.
    pub auto_reconnect: Option<bool>,

    /// Override heartbeat interval.
    pub heartbeat_secs: Option<u64>,

    /// Override retry configuration.
    pub retry: Option<RetryConfig>,
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

/// HTTP client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpClientConfig {
    /// HTTP server URL.
    pub url: String,

    /// Access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Override timeout.
    pub timeout_secs: Option<u64>,

    /// Override retry configuration.
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

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_ws_path() -> String {
    "/ws".to_string()
}

fn default_http_path() -> String {
    "/".to_string()
}
