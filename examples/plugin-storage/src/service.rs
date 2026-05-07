use std::path::{Path, PathBuf};
use std::sync::Arc;

use amira::framework::{context::PluginContext, plugin::ServiceInit};
use amira::macros::service_meta;
use serde::{Deserialize, Serialize};

// ─── StorageService trait ─────────────────────────────────────────────────────

/// Service trait that provides access to the three conventional storage
/// directories used by Amira bots.
#[service_meta("storage")]
pub trait StorageService: Send + Sync {
    /// Returns the `<base>/cache/` directory path.
    fn cache_dir(&self) -> &Path;

    /// Returns the `<base>/data/` directory path.
    fn data_dir(&self) -> &Path;

    /// Returns the `<base>/config/` directory path.
    fn config_dir(&self) -> &Path;
}

/// Configuration for the storage plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Root directory for all storage subdirectories. Defaults to `.`.
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,

    /// Optional overrides for the three storage subdirectories.
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub config_dir: Option<PathBuf>,

    /// Create storage directories if they do not exist. Defaults to `true`.
    #[serde(default = "default_auto_create")]
    pub auto_create: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: default_base_dir(),
            cache_dir: None,
            data_dir: None,
            config_dir: None,
            auto_create: default_auto_create(),
        }
    }
}

fn default_base_dir() -> PathBuf {
    PathBuf::from(".")
}

const fn default_auto_create() -> bool {
    true
}

// ─── StorageServiceImpl ───────────────────────────────────────────────────────

/// Concrete implementation of [`StorageService`], backed by the local filesystem.
///
/// Instantiated by the framework via [`ServiceInit::init`]; you should not
/// construct this directly — consume it through `ServiceRef<dyn StorageService>`.
pub struct StorageServiceImpl {
    cache_dir: PathBuf,
    data_dir: PathBuf,
    config_dir: PathBuf,
}

impl StorageService for StorageServiceImpl {
    fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

impl ServiceInit for StorageServiceImpl {
    /// Constructs the service and creates the three conventional subdirectories.
    ///
    /// Reads `base_dir` from config; falls back to `"."` when absent.
    async fn init(ctx: Arc<PluginContext>) -> Result<Self, String> {
        let cfg: StorageConfig = ctx
            .config()
            .transpose()
            .map_err(|e| format!("Failed to load storage config: {e}"))?
            .unwrap_or_default();

        let base = cfg.base_dir;
        let service = Self {
            cache_dir: cfg.cache_dir.unwrap_or_else(|| base.join("cache")),
            data_dir: cfg.data_dir.unwrap_or_else(|| base.join("data")),
            config_dir: cfg.config_dir.unwrap_or_else(|| base.join("config")),
        };

        if cfg.auto_create {
            for dir in [
                service.cache_dir(),
                service.data_dir(),
                service.config_dir(),
            ] {
                if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                    return Err(format!(
                        "Failed to create storage directory '{}': {}",
                        dir.display(),
                        e
                    ));
                }
            }
        }

        Ok(service)
    }
}
