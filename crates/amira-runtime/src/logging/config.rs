use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    /// Example: `{ "amira_transport": "debug", "hyper": "warn" }`
    #[serde(default)]
    pub filters: HashMap<String, LogLevel>,

    /// Log file path (only used when output is "file").
    #[cfg(feature = "file-logging")]
    pub file_path: Option<std::path::PathBuf>,
}

fn default_timestamps() -> bool {
    true
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
    #[cfg(feature = "json-logging")]
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
    #[cfg(feature = "file-logging")]
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

    /// Enter and exit events.
    pub const ACTIVE: Self = Self {
        new: false,
        enter: true,
        exit: true,
        close: false,
    };
}
