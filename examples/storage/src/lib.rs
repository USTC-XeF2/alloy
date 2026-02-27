//! Built-in file-storage plugin.
//!
//! Exposes the `"alloy.storage"` service via the [`StorageService`] *trait*.
//! The framework automatically instantiates [`StorageServiceImpl`] (which
//! implements the trait) during plugin load and stores it as
//! `Arc<dyn StorageService>` in the global service registry.
//!
//! # Service ID
//!
//! [`STORAGE_SERVICE_ID`] = `"alloy.storage"`
//!
//! # Directories
//!
//! | Method | Path | Purpose |
//! |--------|------|---------|
//! | [`cache_dir`](StorageService::cache_dir) | `<base>/cache/` | Disposable cached data |
//! | [`data_dir`](StorageService::data_dir)   | `<base>/data/`  | Persistent bot state |
//! | [`config_dir`](StorageService::config_dir) | `<base>/config/` | User-editable configs |
//!
//! Configure the base path via `alloy.yaml`:
//!
//! ```yaml
//! plugins:
//!   storage:
//!     base_dir: "./bot_data"
//! ```
//!
//! # Consuming in handlers
//!
//! ```rust,ignore
//! async fn my_handler(
//!     data_dir: StorageDir<Data>,
//!     cache_dir: StorageDir<Cache>,
//! ) -> anyhow::Result<String> {
//!     let state = tokio::fs::read_to_string(
//!         data_dir.join("state.json")
//!     ).await?;
//!     Ok(state)
//! }
//! ```

use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use alloy_framework::{
    context::AlloyContext,
    define_plugin,
    error::{ExtractError, ExtractResult},
    extractor::FromContext,
    plugin::{PluginDescriptor, PluginLoadContext, ServiceInit, ServiceMeta},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::error;

// ─── StorageService trait ─────────────────────────────────────────────────────

/// Service trait that provides access to the three conventional storage
/// directories used by Alloy bots.
pub trait StorageService: Send + Sync + 'static {
    /// Returns the `<base>/cache/` directory path.
    fn cache_dir(&self) -> PathBuf;

    /// Returns the `<base>/data/` directory path.
    fn data_dir(&self) -> PathBuf;

    /// Returns the `<base>/config/` directory path.
    fn config_dir(&self) -> PathBuf;
}

/// Associates the string registry ID with `dyn StorageService`.
///
/// The `define_plugin!` macro reads this constant to populate `provides`
/// and `depends_on` ID arrays.
impl ServiceMeta for dyn StorageService {
    const ID: &'static str = "storage";
}

// ─── Configuration ────────────────────────────────────────────────────────────

/// Configuration for the storage plugin (loaded from `alloy.yaml`).
///
/// ```yaml
/// plugins:
///   storage:
///     base_dir: "./bot_data"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Root directory for all storage subdirectories. Defaults to `.`.
    pub base_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("."),
        }
    }
}

// ─── StorageServiceImpl ───────────────────────────────────────────────────────

/// Concrete implementation of [`StorageService`], backed by the local filesystem.
///
/// Instantiated by the framework via [`ServiceInit::init`]; you should not
/// construct this directly — consume it through `ServiceRef<dyn StorageService>`.
pub struct StorageServiceImpl {
    base_dir: PathBuf,
}

impl StorageServiceImpl {
    /// Creates a new service rooted at `base_dir`.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
}

impl StorageService for StorageServiceImpl {
    fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    fn config_dir(&self) -> PathBuf {
        self.base_dir.join("config")
    }
}

#[async_trait]
impl ServiceInit for StorageServiceImpl {
    /// Constructs the service and creates the three conventional subdirectories.
    ///
    /// Reads `base_dir` from config; falls back to `"."` when absent.
    async fn init(ctx: Arc<PluginLoadContext>) -> Self {
        let cfg: StorageConfig = ctx.get_config().unwrap_or_default();
        let service = StorageServiceImpl::new(&cfg.base_dir);

        let subdirs = ["cache", "data", "config"];
        for sub in &subdirs {
            let dir = service.base_dir.join(sub);
            if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                error!(
                    path  = %dir.display(),
                    error = %e,
                    "Failed to create storage directory"
                );
            }
        }

        service
    }
}

// ─── Directory Kind Selector Trait ───────────────────────────────────────────

/// Trait for selecting a specific directory from [`StorageService`].
///
/// Implemented by marker types ([`Data`], [`Cache`], [`Config`]) to specify
/// which directory to extract when using [`StorageDir<T>`] as a handler parameter.
pub trait StorageDirSelector: 'static {
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
#[derive(Debug, Clone)]
pub struct StorageDir<T: StorageDirSelector>(pub PathBuf, PhantomData<T>);

impl<T: StorageDirSelector> Deref for StorageDir<T> {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: StorageDirSelector> FromContext for StorageDir<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        let storage = ctx
            .get_service::<dyn StorageService>()
            .ok_or_else(|| ExtractError::ServiceNotFound("StorageService"))?;
        Ok(StorageDir(T::select(storage), PhantomData))
    }
}

/// Generic injector for plugin-specific storage directory paths.
///
/// This automatically appends the plugin name to the selected directory.
/// For example, `PluginStorageDir<Data>` returns `<base>/data/<plugin_name>/`.
#[derive(Debug, Clone)]
pub struct PluginStorageDir<T: StorageDirSelector>(pub PathBuf, PhantomData<T>);

impl<T: StorageDirSelector> Deref for PluginStorageDir<T> {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: StorageDirSelector> FromContext for PluginStorageDir<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        let storage = ctx
            .get_service::<dyn StorageService>()
            .ok_or_else(|| ExtractError::ServiceNotFound("StorageService"))?;
        let base_path = T::select(storage);

        let plugin_path = base_path.join(ctx.get_plugin_name());
        Ok(PluginStorageDir(plugin_path, PhantomData))
    }
}

pub static STORAGE_PLUGIN: PluginDescriptor = define_plugin! {
    name: "storage",
    provides: {
        StorageService: StorageServiceImpl,
    },
};
