//! Logging utilities for the Alloy framework.
//!
//! This module provides a unified logging setup using `tracing` and `tracing-subscriber`.
//! It supports configuration-driven initialization and Span Events for observing
//! Service lifecycles in Tower middleware.
//!
//! # Configuration-Based Initialization
//!
//! ```rust,ignore
//! use alloy_runtime::config::AlloyConfig;
//! use alloy_runtime::logging;
//!
//! let config = AlloyConfig::load()?;
//! logging::init_from_config(&config.logging);
//! ```
//!
//! # Manual Initialization
//!
//! ```rust,ignore
//! use alloy_runtime::logging::{LoggingBuilder, SpanEvents};
//!
//! LoggingBuilder::new()
//!     .directive("alloy=debug")
//!     .span_events(SpanEvents::FULL)
//!     .init();
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use tracing::warn;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::TryInitError;
use tracing_subscriber::{EnvFilter, fmt};

use crate::config::{LogFormat, LogOutput, LoggingConfig, SpanEventConfig};

/// Span event configuration for logging.
///
/// This controls when span lifecycle events are logged, which is essential
/// for debugging Tower Service chains and understanding request flow.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpanEvents {
    /// Log when a span is created (entered for the first time).
    pub new: bool,
    /// Log when a span is entered.
    pub enter: bool,
    /// Log when a span is exited.
    pub exit: bool,
    /// Log when a span is closed (dropped).
    pub close: bool,
}

impl SpanEvents {
    /// No span events will be logged.
    pub const NONE: Self = Self {
        new: false,
        enter: false,
        exit: false,
        close: false,
    };

    /// Log span creation and close events.
    ///
    /// This is useful for seeing the lifecycle of Service calls without
    /// too much noise from enter/exit events.
    pub const LIFECYCLE: Self = Self {
        new: true,
        enter: false,
        exit: false,
        close: true,
    };

    /// Log all span events (new, enter, exit, close).
    ///
    /// This provides full visibility into Service execution, useful for
    /// debugging complex middleware chains.
    pub const FULL: Self = Self {
        new: true,
        enter: true,
        exit: true,
        close: true,
    };

    /// Log only enter and exit events.
    ///
    /// This is useful for tracking when Services start and finish
    /// processing without the noise of creation/close events.
    pub const ACTIVE: Self = Self {
        new: false,
        enter: true,
        exit: true,
        close: false,
    };

    /// Convert to `tracing_subscriber::fmt::format::FmtSpan` flags.
    fn to_fmt_span(self) -> fmt::format::FmtSpan {
        let mut span = fmt::format::FmtSpan::NONE;
        if self.new {
            span |= fmt::format::FmtSpan::NEW;
        }
        if self.enter {
            span |= fmt::format::FmtSpan::ENTER;
        }
        if self.exit {
            span |= fmt::format::FmtSpan::EXIT;
        }
        if self.close {
            span |= fmt::format::FmtSpan::CLOSE;
        }
        span
    }
}

impl From<&SpanEventConfig> for SpanEvents {
    fn from(config: &SpanEventConfig) -> Self {
        Self {
            new: config.new,
            enter: config.enter,
            exit: config.exit,
            close: config.close,
        }
    }
}

// =============================================================================
// Configuration-Based Initialization
// =============================================================================

/// Initialize logging from a `LoggingConfig`.
///
/// This is the primary way to initialize logging in Alloy. It reads all settings
/// from the configuration and sets up the tracing subscriber accordingly.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_runtime::config::{AlloyConfig, load_config};
/// use alloy_runtime::logging;
///
/// let config = load_config()?;
/// logging::init_from_config(&config.logging);
/// ```
pub fn init_from_config(config: &LoggingConfig) {
    let builder = LoggingBuilder::from_config(config);

    // Use try_init to avoid panicking if already initialized
    let _ = builder.try_init();
}

// =============================================================================
// Configuration-Based LoggingBuilder
// =============================================================================

/// A builder for configuring logging.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_runtime::logging::{LoggingBuilder, SpanEvents};
/// use tracing::Level;
///
/// // Basic setup with log level
/// LoggingBuilder::new()
///     .with_level(Level::DEBUG)
///     .init();
///
/// // With span events for Tower Service debugging
/// LoggingBuilder::new()
///     .with_level(Level::DEBUG)
///     .with_span_events(SpanEvents::LIFECYCLE)
///     .with_target(true)
///     .with_thread_ids(true)
///     .init();
/// ```
#[derive(Default)]
pub struct LoggingBuilder {
    directives: Vec<String>,
    level: Option<tracing::Level>,
    span_events: SpanEvents,
    format: LogFormat,
    output: LogOutput,
    with_target: bool,
    with_thread_ids: bool,
    with_file: bool,
    with_line_number: bool,
    file_path: Option<PathBuf>,
    max_file_size: u64,
    max_files: usize,
}

impl LoggingBuilder {
    /// Create a new logging builder.
    pub fn new() -> Self {
        Self {
            format: LogFormat::Compact,
            output: LogOutput::Stdout,
            with_target: true,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_files: 5,
            ..Default::default()
        }
    }

    /// Create a LoggingBuilder from a LoggingConfig.
    pub fn from_config(config: &LoggingConfig) -> Self {
        let mut builder = Self::new();

        // Set level
        builder.level = Some(config.level.to_tracing_level());

        // Set format and output
        builder.format = config.format;
        builder.output = config.output;

        // Set span events
        builder.span_events = SpanEvents::from(&config.span_events);

        // Set display options
        builder.with_target = true; // always show target
        builder.with_thread_ids = config.thread_ids;
        builder.with_file = config.file_location;
        builder.with_line_number = config.file_location;

        // Set file options
        builder.file_path.clone_from(&config.file_path);
        builder.max_file_size = config.max_file_size;
        builder.max_files = config.max_files as usize;

        // Add module filters
        for (module, level) in &config.filters {
            builder
                .directives
                .push(format!("{}={}", module, level.as_str()));
        }

        builder
    }

    /// Set the global log level.
    pub fn with_level(mut self, level: tracing::Level) -> Self {
        self.level = Some(level);
        self
    }

    /// Add a filter directive.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.directive("alloy_runtime=debug")
    ///        .directive("alloy_transport=trace")
    /// ```
    pub fn directive(mut self, directive: &str) -> Self {
        self.directives.push(directive.to_string());
        self
    }

    /// Configure span events for Service lifecycle visibility.
    pub fn span_events(mut self, events: SpanEvents) -> Self {
        self.span_events = events;
        self
    }

    /// Alias for `span_events`.
    pub fn with_span_events(mut self, events: SpanEvents) -> Self {
        self.span_events = events;
        self
    }

    /// Set the output format.
    pub fn format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the output destination.
    pub fn output(mut self, output: LogOutput) -> Self {
        self.output = output;
        self
    }

    /// Include the target (module path) in log output.
    pub fn with_target(mut self, enabled: bool) -> Self {
        self.with_target = enabled;
        self
    }

    /// Include thread IDs in log output.
    pub fn with_thread_ids(mut self, enabled: bool) -> Self {
        self.with_thread_ids = enabled;
        self
    }

    /// Include file names in log output.
    pub fn with_file(mut self, enabled: bool) -> Self {
        self.with_file = enabled;
        self
    }

    /// Include line numbers in log output.
    pub fn with_line_number(mut self, enabled: bool) -> Self {
        self.with_line_number = enabled;
        self
    }

    /// Set file path for file output.
    pub fn file_path(mut self, path: PathBuf) -> Self {
        self.file_path = Some(path);
        self
    }

    /// Set maximum file size before rotation.
    pub fn max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set maximum number of rotated files to keep.
    pub fn max_files(mut self, count: usize) -> Self {
        self.max_files = count;
        self
    }

    /// Build the filter from directives.
    fn build_filter(&self) -> EnvFilter {
        // Use tracing::Level's Display implementation (e.g., "INFO" -> lowercase "info")
        let base_level = self.level.unwrap_or(tracing::Level::INFO);
        let base_filter = base_level.to_string().to_lowercase();

        // Check for RUST_LOG environment variable first
        let mut filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&base_filter));

        // Add user-specified directives
        for directive in &self.directives {
            if let Ok(d) = directive.parse() {
                filter = filter.add_directive(d);
            }
        }

        filter
    }

    /// Initialize the logging system.
    pub fn init(self) {
        let _ = self.try_init();
    }

    /// Try to initialize the logging system, returning an error on failure.
    pub fn try_init(self) -> Result<(), TryInitError> {
        let filter = self.build_filter();
        let span_events = self.span_events.to_fmt_span();

        // Macro to reduce repetition when configuring layers (non-JSON formats)
        macro_rules! configure_layer {
            ($layer:expr) => {
                $layer
                    .with_span_events(span_events)
                    .with_target(self.with_target)
                    .with_thread_ids(self.with_thread_ids)
                    .with_file(self.with_file)
                    .with_line_number(self.with_line_number)
            };
        }

        // Helper macro to reduce repetition in format matching
        macro_rules! init_with_writer {
            ($writer:expr) => {
                match &self.format {
                    #[cfg(feature = "json-log")]
                    LogFormat::Json => {
                        let layer = fmt::layer()
                            .json()
                            .with_span_events(span_events)
                            .with_writer($writer);
                        tracing_subscriber::registry()
                            .with(layer)
                            .with(filter)
                            .try_init()
                    }
                    LogFormat::Compact => {
                        let layer = configure_layer!(fmt::layer().compact().with_writer($writer));
                        tracing_subscriber::registry()
                            .with(layer)
                            .with(filter)
                            .try_init()
                    }
                    LogFormat::Full => {
                        let layer = configure_layer!(fmt::layer().with_writer($writer));
                        tracing_subscriber::registry()
                            .with(layer)
                            .with(filter)
                            .try_init()
                    }
                    LogFormat::Pretty => {
                        let layer = configure_layer!(fmt::layer().pretty().with_writer($writer));
                        tracing_subscriber::registry()
                            .with(layer)
                            .with(filter)
                            .try_init()
                    }
                }
            };
        }

        // Choose writer based on output configuration, then apply format
        match &self.output {
            LogOutput::Stdout => init_with_writer!(std::io::stdout),
            LogOutput::Stderr => init_with_writer!(std::io::stderr),
            LogOutput::File => {
                if let Some(path) = self.file_path {
                    let file_appender = tracing_appender::rolling::never(
                        path.parent().unwrap_or_else(|| Path::new(".")),
                        path.file_name().unwrap_or_else(|| OsStr::new("alloy.log")),
                    );
                    init_with_writer!(file_appender)
                } else {
                    warn!(
                        "File output requested but no file path configured, falling back to stdout"
                    );
                    init_with_writer!(std::io::stdout)
                }
            }
        }
    }
}
