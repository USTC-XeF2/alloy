//! Unified error types for the Alloy framework.
//!
//! This module provides standardized error types used across the framework,
//! replacing ad-hoc `anyhow` usage with strongly-typed errors.

use thiserror::Error;

// =============================================================================
// Extraction Errors
// =============================================================================

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

    /// No bot is available in the context.
    #[error("bot not available in context")]
    BotNotAvailable,

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
    /// Transport capability not available.
    #[error("transport capability '{transport}' not available")]
    TransportUnavailable {
        /// The missing transport type.
        transport: &'static str,
    },

    /// Connection setup failed.
    #[error("connection failed to '{url}': {reason}")]
    ConnectionFailed {
        /// The URL that failed.
        url: String,
        /// Reason for failure.
        reason: String,
    },

    /// Adapter initialization failed.
    #[error("initialization failed: {reason}")]
    InitializationFailed {
        /// Reason for failure.
        reason: String,
    },

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

/// Result type for extraction operations.
pub type ExtractResult<T> = Result<T, ExtractError>;

/// Result type for transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Result type for adapter operations.
pub type AdapterResult<T> = Result<T, AdapterError>;

// =============================================================================
// Conversion traits for anyhow compatibility during migration
// =============================================================================

impl From<anyhow::Error> for AdapterError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<anyhow::Error> for TransportError {
    fn from(err: anyhow::Error) -> Self {
        Self::Io(err.to_string())
    }
}
