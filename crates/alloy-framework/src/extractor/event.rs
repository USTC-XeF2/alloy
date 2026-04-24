use std::sync::Arc;

use derive_more::{AsRef, Deref};

use crate::context::HandlerContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use alloy_core::{EventRoot, EventView, Scene};

/// Context wrapper that provides access to extracted event data.
///
/// This is the primary way handlers receive events. Use `Deref` to access
/// fields directly on the wrapped type.
///
/// # Example
///
/// ```rust,ignore
/// #[handler]
/// async fn handler(event: Event<PrivateMessage>) -> Outcome {
///     // Access fields directly via Deref
///     println!("From: {} Message: {}", event.user_id, event.plain_text());
///     
///     // The event can be passed directly to APIs
///     bot.send(event.as_ref(), "reply").await.ok();
///     
///     Outcome::Handled
/// }
/// ```
#[derive(Debug, Clone, Deref, AsRef)]
pub struct Event<T: EventView>(T);

/// Implementation for extracting `Event<T>` where `T: EventView`.
///
/// This enables handlers to request events at any level of the hierarchy
/// through the parent delegation mechanism via `DowngradeAny`
impl<T> FromContext for Event<T>
where
    T: EventView,
    T::Root: Clone,
{
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.event()
            .downcast_ref::<T::Root>()
            .cloned()
            .and_then(T::from_root)
            .map(Event)
            .ok_or_else(|| ExtractError::EventTypeMismatch {
                expected: std::any::type_name::<T>(),
                got: ctx.event().event_id().into(),
            })
    }
}

/// Blanket implementation for extracting the raw event as an `Arc<dyn EventRoot>`.
///
/// This allows handlers to access the full event data without needing to know the
/// specific event type.
impl FromContext for Arc<dyn EventRoot> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(ctx.event().clone())
    }
}

/// Blanket implementation for extracting the event's scene as a `Scene` enum.
///
/// This allows handlers to easily access the context of the event without
/// needing to know the specific event type.
impl FromContext for Scene {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.event()
            .scene()
            .ok_or_else(|| ExtractError::Custom("Scene not found".into()))
    }
}
