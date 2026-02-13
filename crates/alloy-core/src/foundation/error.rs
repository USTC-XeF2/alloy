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

impl AdapterError {
    /// Creates an internal adapter error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Creates a parse error.
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::ParseError { reason: msg.into() }
    }
}

// =============================================================================
// Result Type Aliases
// =============================================================================

/// Result type for transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Result type for adapter operations.
pub type AdapterResult<T> = Result<T, AdapterError>;
