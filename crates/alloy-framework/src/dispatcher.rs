//! Event dispatcher for the Alloy framework.
//!
//! This module provides the [`Dispatcher`], which is responsible for receiving
//! events and distributing them to registered matchers.
//!
//! # Matcher-based Dispatch
//!
//! Unlike the previous handler-centric design, the dispatcher now works with
//! [`Matcher`]s. Each matcher contains multiple handlers and a check rule.
//! When an event is dispatched:
//!
//! 1. Matchers are checked in registration order
//! 2. For each matcher where the check passes, all handlers are executed
//! 3. If a matcher is blocking and its check passed, dispatch stops
//!
//! ```rust,ignore
//! use alloy_framework::{Dispatcher, Matcher};
//!
//! let mut dispatcher = Dispatcher::new();
//!
//! // Add a matcher for message events
//! dispatcher.add(
//!     Matcher::new()
//!         .on::<MessageEvent>()
//!         .block(true)
//!         .handler(echo_handler)
//!         .handler(log_handler)
//! );
//!
//! // Add a catch-all matcher
//! dispatcher.add(
//!     Matcher::new()
//!         .handler(fallback_handler)
//! );
//! ```

use std::sync::Arc;

use tracing::{Level, debug, span};

use crate::context::AlloyContext;
use crate::matcher::Matcher;
use alloy_core::{BoxedBot, BoxedEvent};

/// The central event dispatcher for the Alloy framework.
///
/// The `Dispatcher` maintains a collection of matchers and is responsible for:
/// - Receiving incoming events
/// - Creating execution contexts
/// - Invoking matchers in registration order
/// - Respecting blocking rules
///
/// # Thread Safety
///
/// `Dispatcher` is `Send + Sync` and can be safely shared across threads.
#[derive(Default, Clone)]
pub struct Dispatcher {
    /// The collection of registered matchers.
    matchers: Vec<Matcher>,
}

impl Dispatcher {
    /// Creates a new, empty dispatcher.
    pub fn new() -> Self {
        Self {
            matchers: Vec::new(),
        }
    }

    /// Adds a matcher to this dispatcher.
    ///
    /// Matchers are checked in the order they are added.
    pub fn add(&mut self, matcher: Matcher) {
        self.matchers.push(matcher);
    }

    /// Adds a matcher to this dispatcher (builder pattern).
    pub fn with(mut self, matcher: Matcher) -> Self {
        self.matchers.push(matcher);
        self
    }

    /// Returns the number of registered matchers.
    pub fn matcher_count(&self) -> usize {
        self.matchers.len()
    }

    /// Clears all registered matchers.
    pub fn clear(&mut self) {
        self.matchers.clear();
    }

    /// Dispatches an event to all registered matchers.
    ///
    /// The dispatcher will:
    /// 1. Create an [`AlloyContext`] with the event and bot
    /// 2. Iterate through matchers in registration order
    /// 3. For each matcher where the check passes, execute all handlers
    /// 4. Stop if a blocking matcher's check passed
    ///
    /// # Arguments
    ///
    /// * `event` - The boxed event to dispatch.
    /// * `bot` - The bot associated with this event.
    ///
    /// # Returns
    ///
    /// `true` if any matcher processed the event, `false` otherwise.
    pub async fn dispatch(&self, event: BoxedEvent, bot: BoxedBot) -> bool {
        let event_name = event.event_name();
        let span = span!(Level::DEBUG, "dispatch", event_name = %event_name);
        let _enter = span.enter();

        let ctx = Arc::new(AlloyContext::new(event, bot));
        let mut any_matched = false;

        for matcher in &self.matchers {
            if matcher.execute(Arc::clone(&ctx)).await {
                any_matched = true;

                if matcher.is_blocking() {
                    debug!(
                        matcher = matcher.get_name().unwrap_or("unnamed"),
                        "Blocking matcher matched, stopping dispatch"
                    );
                    break;
                }
            }
        }

        any_matched
    }
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatcher")
            .field("matcher_count", &self.matchers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_core::{ApiError, ApiResult, Bot};
    use alloy_core::{Event, MessageSegment};
    use async_trait::async_trait;
    use serde_json::Value;
    use std::any::{Any, TypeId};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone)]
    struct TestSegment;

    impl MessageSegment for TestSegment {
        fn segment_type(&self) -> &str {
            "text"
        }

        fn as_text(&self) -> Option<&str> {
            Some("")
        }

        fn display(&self) -> String {
            String::new()
        }
    }

    #[derive(Clone)]
    struct TestEvent {
        name: &'static str,
    }

    impl Event for TestEvent {
        fn event_name(&self) -> &'static str {
            self.name
        }

        fn platform(&self) -> &'static str {
            "test"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn downgrade_any(&self, type_id: TypeId) -> Option<Box<dyn Any>> {
            if type_id == TypeId::of::<Self>() {
                Some(Box::new(self.clone()))
            } else {
                None
            }
        }

        type Segment = TestSegment;
    }

    struct MockBot;

    #[async_trait]
    impl Bot for MockBot {
        fn id(&self) -> &str {
            "test-bot"
        }

        async fn call_api(&self, _action: &str, _params: &str) -> ApiResult<Value> {
            Err(ApiError::NotConnected)
        }

        async fn send(&self, _event: &dyn Event, _message: &str) -> ApiResult<i64> {
            Err(ApiError::NotConnected)
        }

        fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
            self
        }
    }

    fn mock_bot() -> BoxedBot {
        Arc::new(MockBot)
    }

    #[tokio::test]
    async fn test_dispatch_no_matchers() {
        let dispatcher = Dispatcher::new();
        let event = BoxedEvent::new(TestEvent { name: "test" });
        let matched = dispatcher.dispatch(event, mock_bot()).await;
        assert!(!matched);
    }

    #[tokio::test]
    async fn test_dispatch_with_matcher() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let mut dispatcher = Dispatcher::new();
        dispatcher.add(Matcher::new().check(|_| true).handler(move || {
            let c = Arc::clone(&counter_clone);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        }));

        let event = BoxedEvent::new(TestEvent { name: "test" });
        let matched = dispatcher.dispatch(event, mock_bot()).await;

        assert!(matched);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_blocking_matcher_stops_dispatch() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter1 = Arc::clone(&counter);
        let counter2 = Arc::clone(&counter);

        let mut dispatcher = Dispatcher::new();

        // First matcher - blocking
        dispatcher.add(Matcher::new().check(|_| true).block(true).handler(move || {
            let c = Arc::clone(&counter1);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        }));

        // Second matcher - should not run
        dispatcher.add(Matcher::new().check(|_| true).handler(move || {
            let c = Arc::clone(&counter2);
            async move {
                c.fetch_add(10, Ordering::SeqCst);
            }
        }));

        let event = BoxedEvent::new(TestEvent { name: "test" });
        dispatcher.dispatch(event, mock_bot()).await;

        // Only first matcher should have run
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_non_blocking_matchers_all_run() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter1 = Arc::clone(&counter);
        let counter2 = Arc::clone(&counter);

        let mut dispatcher = Dispatcher::new();

        // First matcher - not blocking
        dispatcher.add(Matcher::new().check(|_| true).handler(move || {
            let c = Arc::clone(&counter1);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        }));

        // Second matcher - should also run
        dispatcher.add(Matcher::new().check(|_| true).handler(move || {
            let c = Arc::clone(&counter2);
            async move {
                c.fetch_add(10, Ordering::SeqCst);
            }
        }));

        let event = BoxedEvent::new(TestEvent { name: "test" });
        dispatcher.dispatch(event, mock_bot()).await;

        // Both matchers should have run
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }
}
