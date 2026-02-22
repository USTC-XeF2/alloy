//! Built-in file-storage plugin.
//!
//! Provides the `"storage"` service ([`StorageService`]) which exposes
//! three conventional directories under a configurable base path:
//!
//! | Directory | Purpose |
//! |-----------|---------|
//! | `<base>/cache/`  | Disposable cached data |
//! | `<base>/data/`   | Persistent bot state and databases |
//! | `<base>/config/` | User-editable configuration files |
//!
//! Directories are created (recursively) during `on_load`.
//!
//! # Service ID
//!
//! [`STORAGE_SERVICE_ID`] = `"storage"`
//!
//! # Default descriptor (`STORAGE_PLUGIN`)
//!
//! Uses `.` (current working directory) as the base.
//!
//! # Custom base directory
//!
//! ```rust,ignore
//! use alloy_framework::plugin::builtin::STORAGE_SERVICE_ID;
//!
//! runtime.register_plugin(define_plugin! {
//!     name: "storage",
//!     provides: {
//!         STORAGE_SERVICE_ID: StorageService,
//!     },
//!     handlers: [],
//! }).await;
//! ```
//!
//! Or simply configure the base path via `alloy.yaml`:
//!
//! ```yaml
//! plugins:
//!   storage:
//!     base_dir: "./bot_data"
//! ```

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::define_plugin;
use crate::plugin::{PluginDescriptor, PluginService};

pub const STORAGE_SERVICE_ID: &str = "storage";

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
    /// Root directory for all storage subdirectories.
    ///
    /// Defaults to `.` (current working directory).
    pub base_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("."),
        }
    }
}

/// Provides access to the three conventional storage directories.
///
/// Obtain via [`ServiceRef<StorageService>`] after loading [`STORAGE_PLUGIN`]:
///
/// ```rust,ignore
/// let storage: Arc<StorageService> = services.get(STORAGE_SERVICE_ID).unwrap();
/// tokio::fs::write(storage.data_dir().join("state.json"), &bytes).await?;
/// ```
pub struct StorageService {
    base_dir: PathBuf,
}

impl StorageService {
    /// Creates a new service rooted at `base_dir`.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Returns the `<base>/cache/` directory path.
    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    /// Returns the `<base>/data/` directory path.
    pub fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    /// Returns the `<base>/config/` directory path.
    pub fn config_dir(&self) -> PathBuf {
        self.base_dir.join("config")
    }

    /// Returns the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[async_trait]
impl PluginService for StorageService {
    const ID: &'static str = "storage";
    /// Constructs the service and creates the three conventional directories.
    ///
    /// Reads `base_dir` from JSON; falls back to `"."` when absent.
    /// Asynchronously creates the cache, data, and config subdirectories.
    async fn init(config: &serde_json::Value) -> Self {
        let cfg: StorageConfig = serde_json::from_value(config.clone()).unwrap_or_default();
        let service = StorageService::new(&cfg.base_dir);

        // Create the three conventional subdirectories.
        let subdirs = ["cache", "data", "config"];
        for sub in &subdirs {
            let dir = service.base_dir().join(sub);
            if let Err(e) = fs::create_dir_all(&dir).await {
                tracing::error!(
                    path = %dir.display(),
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
///
/// Load with:
///
/// ```rust,ignore
/// use alloy_framework::plugin::builtin::STORAGE_PLUGIN;
/// runtime.register_plugin(STORAGE_PLUGIN).await;
/// ```
pub static STORAGE_PLUGIN: PluginDescriptor = define_plugin! {
    name: "storage",
    provides: [StorageService],
};
