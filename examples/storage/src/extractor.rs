use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use alloy::framework::{context::AlloyContext, error::ExtractResult, extractor::FromContext};
use async_trait::async_trait;

use crate::service::StorageService;

// ─── Directory Kind Selector Trait ───────────────────────────────────────────

/// Trait for selecting a specific directory from [`StorageService`].
///
/// Implemented by marker types ([`Data`], [`Cache`], [`Config`]) to specify
/// which directory to extract when using [`StorageDir<T>`] as a handler parameter.
pub trait StorageDirSelector: Send + 'static {
    /// Extract the directory path from the given storage service.
    fn select(service: Arc<dyn StorageService>) -> PathBuf;
}

/// Marker type for the data directory.
pub struct Data;

impl StorageDirSelector for Data {
    fn select(service: Arc<dyn StorageService>) -> PathBuf {
        service.data_dir()
    }
}

/// Marker type for the cache directory.
pub struct Cache;

impl StorageDirSelector for Cache {
    fn select(service: Arc<dyn StorageService>) -> PathBuf {
        service.cache_dir()
    }
}

/// Marker type for the config directory.
pub struct Config;

impl StorageDirSelector for Config {
    fn select(service: Arc<dyn StorageService>) -> PathBuf {
        service.config_dir()
    }
}

// ─── FromContext Extractors for convenient path injection ─────────────────────

/// Generic injector for storage directory paths.
///
/// Use with type parameters:
/// - [`StorageDir<Data>`] for persistent bot state
/// - [`StorageDir<Cache>`] for disposable cached data
/// - [`StorageDir<Config>`] for user-editable configs
///
/// Implements [`Deref`] to `PathBuf`, so you can use PathBuf methods directly.
///
/// Example:
/// ```rust,ignore
/// async fn my_handler(
///     data_dir: StorageDir<Data>,
/// ) -> anyhow::Result<String> {
///     let state = tokio::fs::read_to_string(
///         data_dir.join("state.json")
///     ).await?;
///     Ok(state)
/// }
/// ```
pub struct StorageDir<T: StorageDirSelector>(pub PathBuf, PhantomData<T>);

impl<T: StorageDirSelector> Deref for StorageDir<T> {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<T: StorageDirSelector> FromContext for StorageDir<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        let storage = ctx.require_service::<dyn StorageService>()?;
        Ok(StorageDir(T::select(storage), PhantomData))
    }
}

/// Generic injector for plugin-specific storage directory paths.
///
/// This automatically appends the plugin name to the selected directory.
/// For example, `PluginStorageDir<Data>` returns `<base>/data/<plugin_name>/`.
pub struct PluginStorageDir<T: StorageDirSelector>(pub PathBuf, PhantomData<T>);

impl<T: StorageDirSelector> Deref for PluginStorageDir<T> {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<T: StorageDirSelector + Send> FromContext for PluginStorageDir<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        let storage = ctx.require_service::<dyn StorageService>()?;
        let plugin_path = T::select(storage).join(ctx.get_plugin_name());
        Ok(PluginStorageDir(plugin_path, PhantomData))
    }
}
