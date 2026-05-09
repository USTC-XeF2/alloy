//! Logging utilities for the Amira framework.
//!
//! This module provides a unified logging setup using `tracing` and `tracing-subscriber`.
//! It supports configuration-driven initialization and Span Events for observing
//! Service lifecycles in Tower middleware.

use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::TryInitError;

mod config;

pub use config::{LogFormat, LogLevel, LogOutput, LoggingConfig, SpanEventConfig};

/// Try to initialize logging from a `LoggingConfig`.
///
/// This is the primary way to initialize logging in Amira. It reads all settings
/// from the configuration and sets up the tracing subscriber accordingly.
pub fn try_init_from_config(config: &LoggingConfig) -> Result<(), TryInitError> {
    let mut filter = Targets::new().with_default(config.level.to_tracing_level());
    for (module, level) in &config.filters {
        filter = filter.with_target(module, level.to_tracing_level());
    }

    let mut span_events = fmt::format::FmtSpan::NONE;
    if config.span_events.new {
        span_events |= fmt::format::FmtSpan::NEW;
    }
    if config.span_events.enter {
        span_events |= fmt::format::FmtSpan::ENTER;
    }
    if config.span_events.exit {
        span_events |= fmt::format::FmtSpan::EXIT;
    }
    if config.span_events.close {
        span_events |= fmt::format::FmtSpan::CLOSE;
    }

    // Helper macro to apply common layer settings
    macro_rules! configure_layer {
        ($layer:expr) => {
            $layer
                .with_span_events(span_events)
                .with_target(true) // Always include target
                .with_thread_ids(config.thread_ids)
                .with_thread_names(config.thread_names)
                .with_file(config.file_location)
                .with_line_number(config.file_location)
        };
    }

    // Initialize based on the output writer and format
    macro_rules! init_with_writer {
        ($writer:expr) => {
            match &config.format {
                #[cfg(feature = "json-logging")]
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

    // Choose writer based on output configuration
    match &config.output {
        LogOutput::Stdout => init_with_writer!(std::io::stdout),
        LogOutput::Stderr => init_with_writer!(std::io::stderr),
        #[cfg(feature = "file-logging")]
        LogOutput::File => {
            if let Some(path) = &config.file_path {
                let file_appender = tracing_appender::rolling::never(
                    path.parent().unwrap_or_else(|| std::path::Path::new(".")),
                    path.file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("amira.log")),
                );
                init_with_writer!(file_appender)
            } else {
                tracing::warn!(
                    "File output requested but no file path configured, falling back to stdout"
                );
                init_with_writer!(std::io::stdout)
            }
        }
    }
}
