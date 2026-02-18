//! Runtime error types.

use thiserror::Error;

/// Errors that can occur during runtime operations.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct RuntimeError(pub String);

/// Result type for runtime operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;
