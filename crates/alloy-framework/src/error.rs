//! Error types for the Alloy framework.

use thiserror::Error;

/// Returned by a filter predicate when an event does **not** match.
///
/// The runtime recognises this error and silently skips the service without
/// logging anything. All other errors are treated as genuine failures.
#[derive(Debug, Clone, Error)]
#[error("event skipped by filter")]
pub struct EventSkipped;

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

    #[error("required state '{0}' not found in context")]
    StateNotFound(&'static str),

    #[error("required service '{0}' not found")]
    ServiceNotFound(&'static str),

    /// Custom extraction error.
    #[error("{0}")]
    Custom(String),
}

/// Result type for extraction operations.
pub type ExtractResult<T> = Result<T, ExtractError>;
