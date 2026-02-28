//! Configuration schema definitions using figment.
//!
//! This module defines the configuration structure for the Alloy framework.
//! The design prioritizes:
//!
//! - **Extensibility**: Adapters can define their own configuration sections
//! - **Decoupling**: Core config is separate from adapter-specific config  
//! - **Multi-source**: Supports files, env vars, and programmatic config
//! - **Type safety**: Strong typing with serde and figment extraction
//! - **Adapter-owned transport configs**: Each adapter defines its own connection
//!   configuration (see e.g. `alloy_adapter_onebot::config`).  There are no
//!   generic transport config types at the framework level.
//!
//! # Configuration Hierarchy
//!
//! ```text
//! AlloyConfig
//! ├── logging: LoggingConfig       # Logging settings
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
//!   retry:
//!     max_retries: 3
//!     initial_delay_ms: 1000
//!   
//! adapters:
//!   onebot:
//!     connections:
//!       - type: ws-client
//!         url: ws://127.0.0.1:8080
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use figment::value::Value;
use serde::{Deserialize, Serialize};

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

    /// Adapter-specific configurations.
    ///
    /// Each adapter registers its own configuration schema.
    /// Example: `adapters.onebot` contains OneBot-specific settings.
    #[serde(default)]
    pub adapters: HashMap<String, Value>,

    /// Plugin-specific configurations.
    ///
    /// Keyed by plugin name (must match the `name` field in the plugin descriptor).
    /// Each entry is deserialised into the plugin's declared `config_type` at load
    /// time and injected into every [`AlloyContext`] for that plugin run.
    ///
    /// ```yaml
    /// plugins:
    ///   echo:
    ///     prefix: "[Bot]"
    ///   alloy.storage:
    ///     base_dir: "./bot_data"
    /// ```
    ///
    /// [`AlloyContext`]: alloy_framework::context::AlloyContext
    #[serde(default)]
    pub plugins: HashMap<String, Value>,
}

// =============================================================================
// Logging Configuration
// =============================================================================

/// Logging configuration.
///
/// Supports multiple output formats, targets, and filtering options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LoggingConfig {
    /// Default log level.
    pub level: LogLevel,

    /// Output format.
    pub format: LogFormat,

    /// Output target.
    pub output: LogOutput,

    /// Whether to include timestamps.
    #[serde(default = "default_timestamps")]
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

fn default_timestamps() -> bool {
    true
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
    /// Compact single-line format.
    #[default]
    Compact,
    /// Full verbose format.
    Full,
    /// Human-readable pretty format.
    Pretty,
    /// JSON format for structured logging.
    #[cfg(feature = "json-log")]
    Json,
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
