use std::any::TypeId;

use derive_more::{AsRef, Deref};

use crate::context::HandlerContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use alloy_core::{BoxedEvent, Event as EventTrait, Scene};

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
///     println!("From: {} Message: {}", event.user_id, event.get_plain_text());
///     
///     // The event can be passed directly to APIs
///     bot.send(event.as_ref(), "reply").await.ok();
///     
///     Outcome::Handled
/// }
/// ```
#[derive(Debug, Clone, Deref, AsRef)]
#[as_ref(dyn EventTrait)]
pub struct Event<T: EventTrait>(T);

/// Implementation for extracting `Event<T>` where `T: EventTrait`.
///
/// This enables handlers to request events at any level of the hierarchy
/// through the parent delegation mechanism via `DowngradeAny`:
///
/// ```rust,ignore
/// use alloy_core::Event;
///
/// // Extract a specific event type
/// async fn on_poke(event: Event<PokeNotifyEvent>) {
///     println!("Target: {}", event.target_id);
/// }
///
/// // Extract an intermediate event type
/// async fn on_notice(event: Event<NoticeEvent>) {
///     println!("Notice: {}", event.event_name());
/// }
/// ```
impl<T: EventTrait> FromContext for Event<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.event()
            .downgrade_any(TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast::<T>().ok())
            .map(|boxed| Event(*boxed))
            .ok_or_else(|| ExtractError::EventTypeMismatch {
                expected: std::any::type_name::<T>(),
                got: ctx.event().event_name(),
            })
    }
}

/// Blanket implementation for extracting the event as a clone of [`BoxedEvent`].
///
/// This is useful when a handler needs to work with any event type
/// without knowing the concrete type at compile time.
impl FromContext for BoxedEvent {
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
            .get_scene()
            .ok_or_else(|| ExtractError::Custom("Scene not found".into()))
    }
}
