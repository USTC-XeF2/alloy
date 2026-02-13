//! Error types for the Alloy framework.

use thiserror::Error;

/// Errors that can occur during context extraction.
#[derive(Debug, Clone, Error)]
pub enum ExtractError {
    /// The event type does not match the expected type.
    #[error("event type mismatch: expected '{expected}', got '{got}'")]
    EventTypeMismatch {
        /// Expected type name.
        expected: &'static str,
        /// Actual type name.
        got: &'static str,
    },

    /// The bot type does not match the expected type.
    #[error("bot type mismatch: expected '{expected}'")]
    BotTypeMismatch {
        /// Expected bot type name.
        expected: &'static str,
    },

    /// Custom extraction error.
    #[error("{0}")]
    Custom(String),
}

impl ExtractError {
    /// Creates a custom extraction error.
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

/// Result type for extraction operations.
pub type ExtractResult<T> = Result<T, ExtractError>;
