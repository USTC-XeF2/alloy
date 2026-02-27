use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;

/// Extractor that provides a handler with its plugin's typed configuration.
///
/// The runtime automatically injects the plugin's raw JSON section from
/// `alloy.yaml → plugins.<plugin_name>` into every [`AlloyContext`] before
/// the handler chain runs.  `PluginConfig<T>` deserialises that JSON into `T`.
///
/// If the config section is absent or empty, `T::default()` is used (requires
/// `T: Default`).  If deserialisation fails the handler is skipped with
/// [`ExtractError::MissingState`].
pub struct PluginConfig<T>(pub T);

impl<T> std::ops::Deref for PluginConfig<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: DeserializeOwned + Default> FromContext for PluginConfig<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        let json = ctx.get_config();
        let t: T = serde_json::from_value((*json).clone()).unwrap_or_default();
        Ok(PluginConfig(t))
    }
}

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
            .ok_or(ExtractError::ServiceNotFound(std::any::type_name::<T>()))
    }
}
