//! Runtime error types.

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

    /// YAML/configuration parsing error.
    #[error("Configuration parse error: {0}")]
    ParseError(String),
}

/// Result type for configuration operations.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Errors that can occur during runtime operations.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct RuntimeError(pub String);

/// Result type for runtime operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;
