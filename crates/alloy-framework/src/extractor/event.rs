use std::any::TypeId;

use async_trait::async_trait;

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use alloy_core::Event as EventTrait;

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
pub struct Event<T: EventTrait>(pub T);

impl<T: EventTrait> Event<T> {
    /// Creates a new Event with the given data.
    pub(crate) fn new(data: T) -> Self {
        Self(data)
    }
}

impl<T: EventTrait> std::ops::Deref for Event<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: EventTrait> AsRef<dyn EventTrait> for Event<T> {
    fn as_ref(&self) -> &dyn EventTrait {
        &self.0
    }
}

impl<T: EventTrait + std::fmt::Debug> std::fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event").field("data", &self.0).finish()
    }
}

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
#[async_trait]
impl<T: EventTrait> FromContext for Event<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.event()
            .downgrade_any(TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast::<T>().ok())
            .map(|boxed| Event::new(*boxed))
            .ok_or_else(|| ExtractError::EventTypeMismatch {
                expected: std::any::type_name::<T>(),
                got: ctx.event().event_name(),
            })
    }
}
