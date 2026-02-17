//! Matcher system for the Alloy framework.
//!
//! A [`Matcher`] is a container that groups multiple handlers together with
//! a common "check" rule. Only when the check passes will the handlers be executed.
//!
//! # Design
//!
//! Unlike the previous design where each handler controlled event propagation,
//! now the Matcher is responsible for:
//! - Checking if the event matches certain criteria
//! - Executing all handlers if the check passes
//! - Controlling whether to block further matchers (via `block` setting)
//!
//! # Tower Service Integration
//!
//! `Matcher` implements `tower::Service<Arc<AlloyContext>>`, allowing you to
//! apply Tower middleware directly:
//!
//! ```rust,ignore
//! use tower::ServiceBuilder;
//! use tower::timeout::TimeoutLayer;
//! use std::time::Duration;
//!
//! let matcher = Matcher::new()
//!     .on::<MessageEvent>()
//!     .handler(echo_handler);
//!
//! // Apply middleware to the matcher
//! let service = ServiceBuilder::new()
//!     .layer(TimeoutLayer::new(Duration::from_secs(5)))
//!     .service(matcher);
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_core::{Matcher, EventContext};
//!
//! // Create a matcher that only handles MessageEvent
//! let matcher = Matcher::new()
//!     .on::<MessageEvent>()  // Check: must be MessageEvent
//!     .block(true)           // Block other matchers after this one
//!     .handler(echo_handler)
//!     .handler(log_handler);
//!
//! // Create a matcher with custom check
//! let matcher = Matcher::new()
//!     .check(|ctx| ctx.event().is::<MessageEvent>())
//!     .handler(my_handler);
//! ```

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tower::Service;
use tracing::{debug, trace};

use crate::context::AlloyContext;
use crate::handler::{BoxedHandler, Handler, into_handler};
use alloy_core::Event;

/// A type-erased check function.
pub type CheckFn = Arc<dyn Fn(&AlloyContext) -> bool + Send + Sync>;

/// Internal data for a Matcher.
///
/// This is wrapped in an `Arc` to enable cheap cloning.
/// Implements `Clone` to support `Arc::make_mut` for copy-on-write semantics.
#[derive(Clone)]
struct MatcherInner {
    /// The check function that determines if this matcher should process the event.
    check_fn: Option<CheckFn>,

    /// The handlers to execute when the check passes.
    handlers: Vec<BoxedHandler>,

    /// Whether to block further matchers after this one processes the event.
    block: bool,

    /// Optional name for debugging.
    name: Option<String>,
}

/// A matcher that groups handlers with a common check rule.
///
/// Handlers within a matcher are executed sequentially when the check passes.
/// The matcher can optionally block further matchers from processing the event.
///
/// # Cheap Cloning
///
/// `Matcher` uses internal `Arc` to enable cheap cloning. This is important
/// for Tower Service integration where cloning may be required.
#[derive(Clone)]
pub struct Matcher {
    inner: Arc<MatcherInner>,
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher {
    /// Creates a new empty matcher.
    ///
    /// By default, a matcher with no check will match all events.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MatcherInner {
                check_fn: None,
                handlers: Vec::new(),
                block: false,
                name: None,
            }),
        }
    }

    /// Internal helper to get mutable access to inner.
    /// Creates a new Arc if there are other references.
    fn inner_mut(&mut self) -> &mut MatcherInner {
        Arc::make_mut(&mut self.inner)
    }

    /// Sets a name for this matcher (useful for debugging).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner_mut().name = Some(name.into());
        self
    }

    /// Sets a custom check function.
    ///
    /// The check function receives the context and returns whether this
    /// matcher should process the event.
    pub fn check<F>(mut self, f: F) -> Self
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static,
    {
        self.inner_mut().check_fn = Some(Arc::new(f));
        self
    }

    /// Sets the check to match events of type `T`.
    ///
    /// This is a convenience method equivalent to:
    /// ```rust,ignore
    /// matcher.check(|ctx| ctx.event().extract::<T>().is_some())
    /// ```
    pub fn on<T>(self) -> Self
    where
        T: Event + Clone + 'static,
    {
        self.check(|ctx| ctx.event().extract::<T>().is_some())
    }

    /// Sets whether this matcher blocks further matchers.
    ///
    /// When `block` is `true`, if this matcher's check passes and handlers
    /// are executed, no further matchers will process the event.
    pub fn block(mut self, block: bool) -> Self {
        self.inner_mut().block = block;
        self
    }

    /// Adds a handler to this matcher.
    ///
    /// Handlers are executed in the order they are added.
    pub fn handler<F, T>(mut self, f: F) -> Self
    where
        F: Handler<T> + Send + Sync + 'static,
        T: 'static,
    {
        self.inner_mut().handlers.push(into_handler(f));
        self
    }

    /// Adds a pre-built boxed handler.
    pub fn handler_boxed(mut self, handler: BoxedHandler) -> Self {
        self.inner_mut().handlers.push(handler);
        self
    }

    /// Checks if this matcher should process the given event.
    pub fn matches(&self, ctx: &AlloyContext) -> bool {
        match &self.inner.check_fn {
            Some(f) => f(ctx),
            None => true, // No check means match all
        }
    }

    /// Returns whether this matcher blocks further matchers.
    pub fn is_blocking(&self) -> bool {
        self.inner.block
    }

    /// Returns the number of handlers in this matcher.
    pub fn handler_count(&self) -> usize {
        self.inner.handlers.len()
    }

    /// Returns the name of this matcher, if set.
    pub fn get_name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    /// Executes all handlers in this matcher.
    ///
    /// Returns `true` if any handler was executed, `false` if the check failed.
    pub async fn execute(&self, ctx: Arc<AlloyContext>) -> bool {
        if !self.matches(&ctx) {
            trace!(
                matcher = self.inner.name.as_deref().unwrap_or("unnamed"),
                "Matcher check failed, skipping"
            );
            return false;
        }

        debug!(
            matcher = self.inner.name.as_deref().unwrap_or("unnamed"),
            handler_count = self.inner.handlers.len(),
            "Matcher check passed, executing handlers"
        );

        for (i, handler) in self.inner.handlers.iter().enumerate() {
            trace!(
                matcher = self.inner.name.as_deref().unwrap_or("unnamed"),
                handler_index = i,
                "Executing handler"
            );
            handler.call(Arc::clone(&ctx)).await;
        }

        true
    }
}

// ============================================================================
// Tower Service Implementation for Matcher
// ============================================================================

/// The response type for Matcher as a Service.
///
/// Contains whether the matcher matched and whether it should block.
#[derive(Debug, Clone, Copy)]
pub struct MatcherResponse {
    /// Whether the matcher's check passed and handlers were executed.
    pub matched: bool,
    /// Whether this matcher is blocking (stops further matchers).
    pub blocking: bool,
}

impl MatcherResponse {
    /// Returns true if matched and blocking.
    pub fn should_stop(&self) -> bool {
        self.matched && self.blocking
    }
}

/// Tower Service implementation for Matcher.
///
/// This allows applying Tower middleware (timeout, rate limiting, etc.)
/// directly to a Matcher.
///
/// # Example
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use tower::timeout::TimeoutLayer;
///
/// let matcher = Matcher::new()
///     .on::<MessageEvent>()
///     .handler(my_handler);
///
/// let service = ServiceBuilder::new()
///     .layer(TimeoutLayer::new(Duration::from_secs(5)))
///     .service(matcher);
/// ```
impl Service<Arc<AlloyContext>> for Matcher {
    type Response = MatcherResponse;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, ctx: Arc<AlloyContext>) -> Self::Future {
        let matcher = self.clone();

        Box::pin(async move {
            let matched = matcher.execute(ctx).await;
            Ok(MatcherResponse {
                matched,
                blocking: matcher.is_blocking(),
            })
        })
    }
}
