//! Configuration error types.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during configuration loading and validation.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// File not found at the specified path.
    #[error("Configuration file not found: {0}")]
    FileNotFound(PathBuf),

    /// Failed to read the configuration file.
    #[error("Failed to read configuration file: {0}")]
    ReadError(#[from] std::io::Error),

    /// YAML parsing error.
    #[error("Failed to parse YAML configuration: {0}")]
    ParseError(#[from] serde_yaml::Error),

    /// Invalid configuration value.
    #[error("Invalid configuration: {message}")]
    ValidationError { message: String },

    /// Missing required field.
    #[error("Missing required configuration field: {field}")]
    MissingField { field: String },

    /// Invalid transport type.
    #[error("Invalid transport type: {0}")]
    InvalidTransportType(String),

    /// Invalid adapter type.
    #[error("Invalid adapter type: {0}")]
    InvalidAdapterType(String),

    /// Duplicate bot identifier.
    #[error("Duplicate bot identifier: {0}")]
    DuplicateBotId(String),

    /// Invalid URL format.
    #[error("Invalid URL: {url} - {reason}")]
    InvalidUrl { url: String, reason: String },

    /// Invalid port number.
    #[error("Invalid port number: {0}")]
    InvalidPort(u16),

    /// Environment variable error.
    #[error("Environment variable error: {0}")]
    EnvVarError(String),
}

impl ConfigError {
    /// Creates a validation error with the given message.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
        }
    }

    /// Creates a missing field error.
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Creates an invalid URL error.
    pub fn invalid_url(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidUrl {
            url: url.into(),
            reason: reason.into(),
        }
    }
}

/// Result type for configuration operations.
pub type ConfigResult<T> = Result<T, ConfigError>;
