//! Matcher builder functions for common event types.
//!
//! This module provides convenient functions for creating matchers that filter
//! based on event type or specific commands.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_core::{on_message, on_command, on_notice, on_request, on_meta};
//!
//! runtime.register_matchers(vec![
//!     on_message().handler(log_handler),
//!     on_command("echo").handler(echo_handler),
//!     on_notice().handler(notice_handler),
//!     on_request().handler(request_handler),
//!     on_meta().handler(meta_handler),
//! ]).await;
//! ```

use crate::foundation::event::EventType;
use crate::framework::matcher::Matcher;

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
    Matcher::new()
        .name("message")
        .check(|ctx| ctx.event().event_type() == EventType::Message)
}

/// Creates a matcher for command events.
///
/// This is a convenience function that automatically:
/// - Sets the matcher name to "command:<cmd>"
/// - Prepends "/" to the command if not present
/// - Filters for message events that start with the command (case-insensitive)
/// - Uses `Event::plain_text()` to extract the message text
///
/// # Example
///
/// ```rust,ignore
/// // Matches messages starting with "/echo"
/// let matcher = on_command("echo")
///     .handler(echo_handler);
/// ```
pub fn on_command(cmd: impl Into<String>) -> Matcher {
    let cmd = cmd.into();
    // Auto-prepend "/" if not present
    let full_cmd = if cmd.starts_with('/') {
        cmd
    } else {
        format!("/{}", cmd)
    };

    let matcher_name = format!("command:{}", full_cmd.trim_start_matches('/'));
    let cmd_check = full_cmd.clone();

    Matcher::new().name(matcher_name).check(move |ctx| {
        if ctx.event().event_type() != EventType::Message {
            return false;
        }
        let text = ctx.event().plain_text();
        text.trim()
            .to_lowercase()
            .starts_with(&cmd_check.to_lowercase())
    })
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
    Matcher::new()
        .name("notice")
        .check(|ctx| ctx.event().event_type() == EventType::Notice)
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
    Matcher::new()
        .name("request")
        .check(|ctx| ctx.event().event_type() == EventType::Request)
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
    Matcher::new()
        .name("meta")
        .check(|ctx| ctx.event().event_type() == EventType::Meta)
}
