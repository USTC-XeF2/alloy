//! File-storage plugin for Alloy.
//!
//! Exposes the `"storage"` service via the [`StorageService`] *trait*.
//! The framework automatically instantiates [`StorageServiceImpl`] (which
//! implements the trait) during plugin load and stores it as
//! `Arc<dyn StorageService>` in the global service registry.
//!
//! # Directories
//!
//! | Method | Path | Purpose |
//! |--------|------|---------|
//! | [`cache_dir`](StorageService::cache_dir) | `<base>/cache/` | Disposable cached data |
//! | [`data_dir`](StorageService::data_dir)   | `<base>/data/`  | Persistent bot state |
//! | [`config_dir`](StorageService::config_dir) | `<base>/config/` | User-editable configs |
//!
//! Configure the base path via `alloy.toml`:
//!
//! ```toml
//! [plugins.storage]
//! base_dir = "./bot_data"
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

mod extractor;
mod service;

use alloy::macros::define_plugin;

pub use extractor::{Cache, Config, Data, PluginStorageDir, StorageDir, StorageDirSelector};
pub use service::{StorageConfig, StorageService, StorageServiceImpl};

define_plugin! {
    /// The storage plugin, providing the `StorageService` for directory access.
    name: "storage",
    provides: {
        StorageService: StorageServiceImpl,
    },
}
