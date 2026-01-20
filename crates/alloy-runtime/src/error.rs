//! Runtime error types.

use thiserror::Error;

/// Errors that can occur during runtime operations.
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),

    /// Bot not found.
    #[error("Bot not found: {0}")]
    BotNotFound(String),

    /// Bot already exists.
    #[error("Bot already exists: {0}")]
    BotExists(String),

    /// Bot is in an invalid state for the requested operation.
    #[error("Invalid bot state: {0}")]
    InvalidState(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for runtime operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;
