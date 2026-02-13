//! Matcher builder functions for common event types.
//!
//! This module provides convenient functions for creating matchers that filter
//! based on event type.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_core::{on_message, on_notice, on_request, on_meta};
//!
//! runtime.register_matchers(vec![
//!     on_message().handler(log_handler),
//!     on_notice().handler(notice_handler),
//!     on_request().handler(request_handler),
//!     on_meta().handler(meta_handler),
//! ]).await;
//! ```

use crate::matcher::Matcher;
use alloy_core::EventType;

/// Creates a matcher that only handles message events.
///
/// This is a convenience function that automatically:
/// - Sets the matcher name to "message"
/// - Filters for events with `event_type() == EventType::Message`
///
/// # Example
///
/// ```rust,ignore
/// let matcher = on_message()
///     .handler(echo_handler)
///     .handler(log_handler);
/// ```
pub fn on_message() -> Matcher {
    Matcher::new().check(|ctx| ctx.event().event_type() == EventType::Message)
}

/// Creates a matcher that only handles notice events.
///
/// This is a convenience function that automatically:
/// - Sets the matcher name to "notice"
/// - Filters for events with `event_type() == EventType::Notice`
///
/// # Example
///
/// ```rust,ignore
/// let matcher = on_notice()
///     .handler(notice_handler);
/// ```
pub fn on_notice() -> Matcher {
    Matcher::new().check(|ctx| ctx.event().event_type() == EventType::Notice)
}

/// Creates a matcher that only handles request events.
///
/// This is a convenience function that automatically:
/// - Sets the matcher name to "request"
/// - Filters for events with `event_type() == EventType::Request`
///
/// # Example
///
/// ```rust,ignore
/// let matcher = on_request()
///     .handler(request_handler);
/// ```
pub fn on_request() -> Matcher {
    Matcher::new().check(|ctx| ctx.event().event_type() == EventType::Request)
}

/// Creates a matcher that only handles meta events.
///
/// This is a convenience function that automatically:
/// - Sets the matcher name to "meta"
/// - Filters for events with `event_type() == EventType::Meta`
///
/// # Example
///
/// ```rust,ignore
/// let matcher = on_meta()
///     .handler(meta_handler);
/// ```
pub fn on_meta() -> Matcher {
    Matcher::new().check(|ctx| ctx.event().event_type() == EventType::Meta)
}
