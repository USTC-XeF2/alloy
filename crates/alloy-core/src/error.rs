//! Unified error types for the Alloy core framework.
//!
//! This module provides standardized error types used across core components.
//! Framework-level errors (like ExtractError) are defined in alloy-framework.

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

    /// Connection closed.
    #[error("connection closed: {reason}")]
    ConnectionClosed {
        /// Reason for closure.
        reason: String,
    },

    /// Bot identification failed.
    #[error("bot identification failed: {reason}")]
    BotIdMissing {
        /// Reason for failure.
        reason: String,
    },

    /// Message send failed.
    #[error("failed to send message: {0}")]
    SendFailed(String),

    /// Transport not available.
    #[error("transport '{transport}' not available")]
    NotAvailable {
        /// The transport type that's not available.
        transport: &'static str,
    },

    /// Invalid configuration.
    #[error("invalid transport configuration: {0}")]
    InvalidConfig(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// Bot already exists.
    #[error("bot with ID '{id}' already exists")]
    BotAlreadyExists {
        /// The duplicate bot ID.
        id: String,
    },

    /// Bot not found.
    #[error("bot '{id}' not found")]
    BotNotFound {
        /// The missing bot ID.
        id: String,
    },
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
    /// The bot is not connected.
    #[error("bot is not connected")]
    NotConnected,
    /// The API call timed out.
    #[error("API call timed out")]
    Timeout,
    /// The API returned an error.
    #[error("API error ({retcode}): {message}")]
    ApiError { retcode: i64, message: String },
    /// Failed to serialize/deserialize.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// Transport error.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The event does not have the required session information.
    #[error("missing session info")]
    MissingSession,
    /// Other error.
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
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
