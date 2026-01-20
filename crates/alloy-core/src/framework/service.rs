//! Tower Service types for the Alloy framework.
//!
//! This module provides common types used with Tower middleware.
//!
//! # Service Architecture
//!
//! The primary Service in Alloy is [`Matcher`](super::matcher::Matcher), which
//! implements `tower::Service<Arc<AlloyContext>>`. You can apply Tower middleware
//! directly to matchers:
//!
//! ```rust,ignore
//! use tower::ServiceBuilder;
//! use tower::timeout::TimeoutLayer;
//!
//! let matcher = Matcher::new()
//!     .on::<MessageEvent>()
//!     .handler(my_handler);
//!
//! let service = ServiceBuilder::new()
//!     .layer(TimeoutLayer::new(Duration::from_secs(5)))
//!     .service(matcher);
//! ```

use std::future::Future;
use std::pin::Pin;

/// Error type for Alloy services.
#[derive(Debug, thiserror::Error)]
pub enum AlloyError {
    /// Handler execution timed out.
    #[error("handler timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Rate limit exceeded.
    #[error("rate limit exceeded for {0}")]
    RateLimited(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Circuit breaker is open.
    #[error("circuit breaker open: {0}")]
    CircuitOpen(String),

    /// Internal handler error.
    #[error("handler error: {0}")]
    Handler(#[from] anyhow::Error),

    /// Service unavailable.
    #[error("service unavailable: {0}")]
    Unavailable(String),
}

/// A boxed future type for service responses.
pub type ServiceFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

/// A bot command for outbound API calls.
///
/// This represents a command to be sent to the bot backend.
#[derive(Debug, Clone)]
pub struct BotCommand {
    /// The API endpoint/action name.
    pub action: String,
    /// Command parameters.
    pub params: serde_json::Value,
    /// Optional echo identifier for request tracking.
    pub echo: Option<String>,
}

impl BotCommand {
    /// Creates a new bot command.
    pub fn new(action: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            action: action.into(),
            params,
            echo: None,
        }
    }

    /// Sets the echo identifier for request tracking.
    pub fn with_echo(mut self, echo: impl Into<String>) -> Self {
        self.echo = Some(echo.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_command() {
        let cmd = BotCommand::new("send_message", serde_json::json!({"message": "hello"}))
            .with_echo("test-123");

        assert_eq!(cmd.action, "send_message");
        assert_eq!(cmd.echo, Some("test-123".to_string()));
    }
}
