//! Unified error types for the Amira core framework.
//!
//! This module provides standardized error types used across core components.
//! Framework-level errors (like ExtractError) are defined in amira-framework.

use thiserror::Error;

// =============================================================================
// Transport Errors
// =============================================================================

/// Errors that can occur in transport operations.
#[derive(Debug, Clone, Error)]
pub enum TransportError {
    /// Connection failed.
    #[error("connection failed: {url} - {reason}")]
    ConnectionFailed {
        /// The URL that failed to connect.
        url: String,
        /// Reason for failure.
        reason: String,
    },

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

// =============================================================================
// Adapter Errors
// =============================================================================

/// Errors that can occur in adapter operations.
#[derive(Debug, Clone, Error)]
pub enum AdapterError {
    /// Event parsing failed.
    #[error("failed to parse event: {reason}")]
    ParseError {
        /// Reason for failure.
        reason: String,
    },

    /// Internal adapter error.
    #[error("adapter error: {0}")]
    Internal(String),

    /// Transport error.
    #[error(transparent)]
    Transport(#[from] TransportError),
}

// =============================================================================
// API Errors
// =============================================================================

/// Error type for API calls.
#[derive(Debug, Clone, Error)]
pub enum ApiError {
    /// The API call timed out.
    #[error("API call timed out")]
    Timeout,
    /// The transport does not support API calls.
    #[error("API call not supported by this transport")]
    NotSupported,
    /// The API returned an error.
    #[error("API error ({retcode}): {message}")]
    ApiError { retcode: i64, message: String },
    /// Failed to serialize/deserialize.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// Transport error.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// Other error.
    #[error("{0}")]
    Other(String),
}

impl<E: serde::ser::Error> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self::SerializationError(err.to_string())
    }
}

// =============================================================================
// Result Type Aliases
// =============================================================================

/// Result type for transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Result type for adapter operations.
pub type AdapterResult<T> = Result<T, AdapterError>;

/// Result type for API calls.
pub type ApiResult<T> = Result<T, ApiError>;
