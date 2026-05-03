//! Tower middleware layers for filtering and routing events.
//!
//! This module provides convenience functions that return pre-configured
//! [`ServiceBuilder`]s with filter layers stacked. The predicates use
//! [`EventPredicate`] from `service` module which returns [`EventSkipped`]
//! on mismatch, and the runtime silently ignores these errors.
//!
//! # Example
//!
//! ```rust,ignore
//! use amira_framework::{on_message, ServiceBuilderExt};
//!
//! let svc = on_message().handler(my_handler);
//! runtime.register_service(svc).await;
//! ```

use amira_core::{EventType, EventView};
use tower::ServiceBuilder;
use tower::filter::FilterLayer;
use tower_layer::{Identity, Stack};

use crate::context::HandlerContext;
use crate::handler::{EventPredicate, ServiceBuilderExt};

/// Convenience type alias for the `ServiceBuilder` returned by `on_message()`,
/// `on_event_type()`, and `on()`.
pub type FilterServiceBuilder = ServiceBuilder<Stack<FilterLayer<EventPredicate>, Identity>>;

/// Creates a [`ServiceBuilder`] that filters events by [`EventType`].
pub fn on_event_type(event_type: EventType) -> FilterServiceBuilder {
    ServiceBuilder::new()
        .rule_sync(move |ctx: &HandlerContext| ctx.event().event_type() == event_type)
}

/// Creates a [`ServiceBuilder`] that only passes through **message** events.
///
/// # Example
///
/// ```rust,ignore
/// use amira::prelude::*;
///
/// runtime.register_service(on_message().handler(my_handler)).await;
/// ```
pub fn on_message() -> FilterServiceBuilder {
    on_event_type(EventType::Message)
}

/// Creates a [`ServiceBuilder`] that filters events to a specific concrete
/// event type `E`.
///
/// Uses strict type equality checking.
pub fn on<E>() -> FilterServiceBuilder
where
    E: EventView,
    E::Root: Clone,
{
    ServiceBuilder::new().rule_sync(move |ctx: &HandlerContext| {
        ctx.event()
            .downcast_ref::<E::Root>()
            .cloned()
            .and_then(E::from_root)
            .is_some()
    })
}
