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
//! # Loading
//!
//! ```rust,ignore
//! use alloy_framework::plugin::builtin::STORAGE_PLUGIN;
//! runtime.register_plugin(STORAGE_PLUGIN).await;
//! ```
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
//!     storage: ServiceRef<dyn StorageService>,
//! ) -> anyhow::Result<String> {
//!     let state = tokio::fs::read_to_string(
//!         storage.data_dir().join("state.json")
//!     ).await?;
//!     Ok(state)
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::define_plugin;
use crate::plugin::{PluginDescriptor, PluginLoadContext, ServiceInit, ServiceMeta};

/// Unique registry ID for the storage service.
pub const STORAGE_SERVICE_ID: &str = "alloy.storage";

// ─── StorageService trait ─────────────────────────────────────────────────────

/// Service trait that provides access to the three conventional storage
/// directories used by Alloy bots.
///
/// Obtain a reference via [`ServiceRef<dyn StorageService>`] in a handler, or
/// through [`AlloyContext::get_service::<dyn StorageService>()`].
///
/// [`AlloyContext::get_service::<dyn StorageService>()`]: crate::context::AlloyContext::get_service
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
    const ID: &'static str = STORAGE_SERVICE_ID;
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
                tracing::error!(
                    path  = %dir.display(),
                    error = %e,
                    "Failed to create storage directory"
                );
            }
        }

        service
    }
}

// ─── Plugin descriptor ────────────────────────────────────────────────────────

/// Static descriptor for the built-in file-storage plugin.
///
/// Uses `./` as the base path by default.  Override via `alloy.yaml`:
///
/// ```yaml
/// plugins:
///   storage:
///     base_dir: "./bot_data"
/// ```
pub static STORAGE_PLUGIN: PluginDescriptor = define_plugin! {
    name: "storage",
    provides: {
        StorageService: StorageServiceImpl,
    },
};
