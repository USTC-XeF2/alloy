//! Runtime error types.

use thiserror::Error;

/// Errors that can occur during runtime operations.
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Adapter configuration deserialization failed.
    #[error("Failed to deserialize adapter config: {0}")]
    AdapterConfigDeserialize(String),

    /// Adapter error.
    #[error("Adapter error: {0}")]
    Adapter(#[from] alloy_core::AdapterError),
}

/// Result type for runtime operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;
