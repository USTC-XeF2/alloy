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
//! use alloy_framework::{on_message, ServiceBuilderExt};
//!
//! let svc = on_message().handler(my_handler);
//! runtime.register_service(svc).await;
//! ```

use std::any::TypeId;

use tower::ServiceBuilder;
use tower::filter::FilterLayer;
use tower_layer::{Identity, Stack};

use alloy_core::{Event, EventType};

use crate::context::AlloyContext;
use crate::handler::{EventPredicate, ServiceBuilderExt};

/// Convenience type alias for the `ServiceBuilder` returned by `on_message()`,
/// `on_event_type()`, and `on()`.
pub type FilterServiceBuilder = ServiceBuilder<Stack<FilterLayer<EventPredicate>, Identity>>;

/// Creates a [`ServiceBuilder`] that filters events by [`EventType`].
///
/// # Example
///
/// ```rust,ignore
/// use alloy::prelude::*;
/// use alloy_core::EventType;
///
/// runtime.register_service(on_event_type(EventType::Message).handler(handler)).await;
/// ```
pub fn on_event_type(event_type: EventType) -> FilterServiceBuilder {
    ServiceBuilder::new().rule(move |ctx: &AlloyContext| ctx.event().event_type() == event_type)
}

/// Creates a [`ServiceBuilder`] that only passes through **message** events.
///
/// # Example
///
/// ```rust,ignore
/// use alloy::prelude::*;
///
/// runtime.register_service(on_message().handler(echo_handler)).await;
/// ```
pub fn on_message() -> FilterServiceBuilder {
    on_event_type(EventType::Message)
}

/// Creates a [`ServiceBuilder`] that filters events to a specific concrete
/// event type `E`.
///
/// Uses strict type equality checking.
///
/// # Example
///
/// ```rust,ignore
/// use alloy::prelude::*;
/// use onebot::events::MessageEvent;
///
/// runtime.register_service(on::<MessageEvent>().handler(handler)).await;
/// ```
pub fn on<E: Event + 'static>() -> FilterServiceBuilder {
    let type_id = TypeId::of::<E>();
    ServiceBuilder::new().rule(move |ctx: &AlloyContext| ctx.event().as_any().type_id() == type_id)
}
