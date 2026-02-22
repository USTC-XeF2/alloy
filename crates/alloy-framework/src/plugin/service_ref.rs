//! [`ServiceRef<T>`] — handler extractor for inter-plugin services.

use std::sync::Arc;

use crate::{
    context::AlloyContext,
    error::{ExtractError, ExtractResult},
    extractor::FromContext,
    plugin::PluginService,
};

/// Extractor that injects a reference to a registered service into a handler.
///
/// If the service `T` has not been registered (e.g. the plugin that provides it
/// was not loaded), extraction fails with [`ExtractError::MissingState`] and
/// the handler is silently skipped.
///
/// The inner [`Arc<T>`] is accessible via [`Deref`] or the `.0` field.
///
/// # Example
///
/// ```rust,ignore
/// async fn signin_handler(
///     event: EventContext<MessageEvent>,
///     storage: ServiceRef<StorageService>,
/// ) -> anyhow::Result<String> {
///     let path = storage.data_dir().join("signin.json");
///     // …
///     Ok("签到成功！".to_string())
/// }
/// ```
pub struct ServiceRef<T>(pub Arc<T>);

impl<T> std::ops::Deref for ServiceRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: PluginService> FromContext for ServiceRef<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.get_service::<T>()
            .map(ServiceRef)
            .ok_or(ExtractError::MissingState)
    }
}
