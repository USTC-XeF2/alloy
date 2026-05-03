use std::sync::Arc;

use derive_more::{AsRef, Deref, DerefMut};

use crate::context::HandlerContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;

/// Extractor that provides a handler with its plugin's typed configuration.
///
/// The runtime automatically injects the plugin's raw JSON section from
/// `amira.toml → plugins.<plugin_name>` into every [`HandlerContext`] before
/// the handler chain runs.  `PluginConfig<T>` deserializes that JSON into `T`.
///
/// If the config section is absent or empty, `T::default()` is used (requires
/// `T: Default`).  If deserialization fails the handler is skipped with
/// [`ExtractError::MissingState`].
#[derive(Deref, AsRef)]
pub struct PluginConfig<T>(T);

impl<T: serde::de::DeserializeOwned + Default + Send> FromContext for PluginConfig<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(PluginConfig(ctx.plugin().config().unwrap_or_default()))
    }
}

/// Extractor that provides access to the plugin's persistent state.
///
/// The plugin context maintains a [`State`](crate::context::State) that persists
/// across all event dispatches for a given plugin instance. `PluginState<T>` allows
/// handlers to extract typed values from this persistent storage.
///
/// Values in plugin state can be set via `ctx.plugin().state().set(value)` and
/// retrieved via this extractor. If a value of type `T` has not been set, extraction
/// fails with [`ExtractError::StateNotFound`] and the handler is silently skipped.
#[derive(Deref, DerefMut)]
pub struct PluginState<T: Clone + Send + 'static>(pub T);

impl<T: Clone + Send + 'static> FromContext for PluginState<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.plugin()
            .state()
            .get::<T>()
            .map(Self)
            .ok_or_else(|| ExtractError::StateNotFound(std::any::type_name::<T>()))
    }
}

/// Extractor that provides mutable access to a plugin state value, with a default.
///
/// This is a convenience wrapper around `PluginState` that allows handlers to easily
/// access a mutable plugin state value that has a default. If the state value of type
/// `T` has not been set, it is initialized with `T::default()` and then returned.
#[derive(Deref, DerefMut)]
pub struct DefaultPluginState<T: Clone + Default + Send + 'static>(pub T);

impl<T: Clone + Default + Send + 'static> FromContext for DefaultPluginState<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(Self(ctx.plugin().state().get_or_insert_with(T::default)))
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
///     event: Event<MessageEvent>,
///     service: ServiceRef<dyn MyService>,
/// ) -> anyhow::Result<String> {
///     let value = service.get_value();
///     // …
///     Ok(value)
/// }
/// ```
#[derive(Deref)]
pub struct ServiceRef<T: ?Sized>(Arc<T>);

impl<T: ?Sized + Send + Sync + 'static> FromContext for ServiceRef<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.require_service::<T>().map(ServiceRef)
    }
}
