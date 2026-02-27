use std::path::PathBuf;
use std::sync::Arc;

use alloy::framework::plugin::{PluginLoadContext, ServiceInit};
use alloy::macros::service_meta;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::error;

// ─── StorageService trait ─────────────────────────────────────────────────────

/// Service trait that provides access to the three conventional storage
/// directories used by Alloy bots.
#[service_meta("storage")]
pub trait StorageService: Send + Sync {
    /// Returns the `<base>/cache/` directory path.
    fn cache_dir(&self) -> PathBuf;

    /// Returns the `<base>/data/` directory path.
    fn data_dir(&self) -> PathBuf;

    /// Returns the `<base>/config/` directory path.
    fn config_dir(&self) -> PathBuf;
}

/// Configuration for the storage plugin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StorageConfig {
    /// Root directory for all storage subdirectories. Defaults to `.`.
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,
}

fn default_base_dir() -> PathBuf {
    PathBuf::from(".")
}

// ─── StorageServiceImpl ───────────────────────────────────────────────────────

/// Concrete implementation of [`StorageService`], backed by the local filesystem.
///
/// Instantiated by the framework via [`ServiceInit::init`]; you should not
/// construct this directly — consume it through `ServiceRef<dyn StorageService>`.
pub struct StorageServiceImpl {
    base_dir: PathBuf,
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
        let service = StorageServiceImpl {
            base_dir: cfg.base_dir,
        };

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
