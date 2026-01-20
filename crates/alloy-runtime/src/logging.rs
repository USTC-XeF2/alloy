//! Logging utilities for the Alloy framework.
//!
//! This module provides a unified logging setup using `tracing` and `tracing-subscriber`.
//! It supports Span Events for observing Service lifecycles in Tower middleware.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_utils::logging::{LoggingBuilder, SpanEvents};
//!
//! fn main() {
//!     // Initialize with default settings
//!     LoggingBuilder::new()
//!         .init();
//!
//!     // Or with span events for Tower Service lifecycle visibility
//!     LoggingBuilder::new()
//!         .directive("alloy=debug")
//!         .span_events(SpanEvents::FULL)
//!         .init();
//! }
//! ```

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

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

/// Initialize logging with default settings.
///
/// This sets up a tracing subscriber with:
/// - Environment-based filtering via `RUST_LOG`
/// - Default directive: `info`
/// - Pretty formatting with timestamps
///
/// # Panics
///
/// Panics if the subscriber has already been set.
pub fn init() {
    init_with_filter("info");
}

/// Initialize logging with a custom filter string.
///
/// # Arguments
///
/// * `filter` - A filter string like `"alloy=debug,my_module=trace"`
///
/// # Example
///
/// ```rust,ignore
/// logging::init_with_filter("alloy_runtime=debug,alloy_transport=trace");
/// ```
///
/// # Panics
///
/// Panics if the subscriber has already been set.
pub fn init_with_filter(filter: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();
}

/// Try to initialize logging, returning an error instead of panicking.
///
/// This is useful when you're not sure if logging has already been initialized.
pub fn try_init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    try_init_with_filter("info")
}

/// Try to initialize logging with a custom filter.
pub fn try_init_with_filter(filter: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .try_init()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

/// Creates a default EnvFilter for Alloy components.
///
/// Returns a filter with sensible defaults for Alloy components:
/// - `alloy_runtime=info`
/// - `alloy_transport=info`
/// - `alloy_adapter_onebot=info`
/// - `alloy_core=debug`
pub fn default_alloy_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info")
            .add_directive("alloy_runtime=info".parse().unwrap())
            .add_directive("alloy_transport=info".parse().unwrap())
            .add_directive("alloy_adapter_onebot=info".parse().unwrap())
            .add_directive("alloy_core=debug".parse().unwrap())
    })
}

/// Initialize logging with Alloy defaults.
///
/// This sets up logging with sensible defaults for all Alloy components.
pub fn init_alloy() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(default_alloy_filter())
        .init();
}

/// A builder for configuring logging.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_utils::logging::{LoggingBuilder, SpanEvents};
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
    with_target: bool,
    with_thread_ids: bool,
    with_file: bool,
    with_line_number: bool,
    #[cfg(feature = "json")]
    json: bool,
}

impl LoggingBuilder {
    /// Create a new logging builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the global log level.
    ///
    /// This sets the minimum level for all log output.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use tracing::Level;
    ///
    /// builder.with_level(Level::DEBUG)
    /// ```
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
    ///
    /// This is essential for debugging Tower middleware chains.
    /// Use `SpanEvents::LIFECYCLE` to see when Services start and complete.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // See when spans are created and closed
    /// builder.span_events(SpanEvents::LIFECYCLE)
    ///
    /// // Full visibility into span enter/exit
    /// builder.span_events(SpanEvents::FULL)
    /// ```
    pub fn span_events(mut self, events: SpanEvents) -> Self {
        self.span_events = events;
        self
    }

    /// Alias for `span_events` - configure span events.
    ///
    /// This is provided for API consistency with other "with_" prefixed methods.
    pub fn with_span_events(mut self, events: SpanEvents) -> Self {
        self.span_events = events;
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

    /// Enable JSON output format.
    #[cfg(feature = "json")]
    pub fn json(mut self) -> Self {
        self.json = true;
        self
    }

    /// Build the filter from directives.
    fn build_filter(&self) -> EnvFilter {
        // Start with the base level or default
        let base_filter = if let Some(level) = self.level {
            let level_str = match level {
                tracing::Level::TRACE => "trace",
                tracing::Level::DEBUG => "debug",
                tracing::Level::INFO => "info",
                tracing::Level::WARN => "warn",
                tracing::Level::ERROR => "error",
            };
            level_str.to_string()
        } else {
            "info".to_string()
        };

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

    /// Build the fmt layer with configured options.
    fn build_fmt_layer<S>(&self) -> fmt::Layer<S>
    where
        S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        fmt::layer()
            .with_span_events(self.span_events.to_fmt_span())
            .with_target(self.with_target)
            .with_thread_ids(self.with_thread_ids)
            .with_file(self.with_file)
            .with_line_number(self.with_line_number)
    }

    /// Initialize the logging system.
    pub fn init(self) {
        let filter = self.build_filter();

        #[cfg(feature = "json")]
        if self.json {
            tracing_subscriber::registry()
                .with(
                    fmt::layer()
                        .json()
                        .with_span_events(self.span_events.to_fmt_span()),
                )
                .with(filter)
                .init();
            return;
        }

        tracing_subscriber::registry()
            .with(self.build_fmt_layer())
            .with(filter)
            .init();
    }

    /// Try to initialize the logging system, returning an error on failure.
    pub fn try_init(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let filter = self.build_filter();

        #[cfg(feature = "json")]
        if self.json {
            return tracing_subscriber::registry()
                .with(
                    fmt::layer()
                        .json()
                        .with_span_events(self.span_events.to_fmt_span()),
                )
                .with(filter)
                .try_init()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
        }

        tracing_subscriber::registry()
            .with(self.build_fmt_layer())
            .with(filter)
            .try_init()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}
