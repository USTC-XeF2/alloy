//! [`ServiceRef<T>`] — handler extractor for inter-plugin services.

use std::sync::Arc;

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;

/// Extractor that injects a reference to a registered service trait object
/// into a handler.
///
/// `T` should be a `dyn ServiceTrait` — the extractor looks up the service by
/// `TypeId::of::<T>()` and returns the stored `Arc<dyn ServiceTrait>`.
///
/// If the service has not been registered (e.g. the plugin that provides it was
/// not loaded), extraction fails with [`ExtractError::MissingState`] and the
/// handler is silently skipped.
///
/// # Example
///
/// ```rust,ignore
/// async fn my_handler(
///     event: EventContext<MessageEvent>,
///     service: ServiceRef<dyn MyService>,
/// ) -> anyhow::Result<String> {
///     let value = service.get_value();
///     // …
///     Ok(value)
/// }
/// ```
pub struct ServiceRef<T: ?Sized>(pub Arc<T>);

impl<T: ?Sized> std::ops::Deref for ServiceRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized + 'static> FromContext for ServiceRef<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.get_service::<T>()
            .map(ServiceRef)
            .ok_or(ExtractError::MissingState)
    }
}
