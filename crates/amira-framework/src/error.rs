//! Error types for the Amira framework.

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
        got: String,
    },

    /// The bot type does not match the expected type.
    #[error("bot type mismatch: expected '{expected}'")]
    BotTypeMismatch {
        /// Expected bot type name.
        expected: &'static str,
    },

    #[error("required state '{0}' not found in context")]
    StateNotFound(&'static str),

    #[error("required service '{0}' not found")]
    ServiceNotFound(&'static str),

    /// Custom extraction error.
    #[error("{0}")]
    Custom(String),
}

/// Returned by a filter predicate when an event does **not** match or
/// when context extraction fails.
#[derive(Debug, Clone, Error)]
pub enum EventSkipped {
    #[error("event skipped by filter")]
    Filter,

    #[error("event skipped due to context extraction failure: {0}")]
    Extract(#[from] ExtractError),
}

/// Result type for extraction operations.
pub type ExtractResult<T> = Result<T, ExtractError>;
