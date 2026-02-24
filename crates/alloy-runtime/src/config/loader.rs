//! Configuration loader using figment.
//!
//! This module provides a flexible configuration loading system that supports:
//!
//! - **Multiple sources**: TOML/YAML files, environment variables, programmatic defaults
//! - **Layered configuration**: Later sources override earlier ones
//! - **Profile support**: Development vs production configurations
//!
//! # Feature Flags
//!
//! - `toml-config` *(default)*: enables TOML configuration files (`alloy.toml`, `config.toml`)
//! - `yaml-config`: enables YAML configuration files (`alloy.yaml`, `alloy.yml`, etc.)
//!
//! Both features can be enabled simultaneously; if so, both file formats are searched and loaded.
//!
//! # Configuration Priority (lowest to highest)
//!
//! 1. Built-in defaults
//! 2. Profile-specific config file (`alloy.{profile}.toml` / `alloy.{profile}.yaml`)
//! 3. Main config file (`alloy.toml` / `alloy.yaml`)
//! 4. Environment variables (`ALLOY_*`)
//! 5. Programmatic overrides
//!
//! # Environment Variable Mapping
//!
//! Environment variables are mapped using the `ALLOY_` prefix with `__` as separator:
//!
//! - `ALLOY_LOGGING__LEVEL=debug` → `logging.level = "debug"`
//! - `ALLOY_NETWORK__TIMEOUT_SECS=60` → `network.timeout_secs = 60`
//! - `ALLOY_ADAPTERS__ONEBOT__ACCESS_TOKEN=xxx` → `adapters.onebot.access_token = "xxx"`
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_runtime::config::{ConfigLoader, AlloyConfig};
//!
//! // Simple loading from default locations
//! let config = ConfigLoader::new().load()?;
//!
//! // Load with specific profile
//! let config = ConfigLoader::new()
//!     .profile("production")
//!     .load()?;
//!
//! // Load from specific file with env overrides
//! let config = ConfigLoader::new()
//!     .file("./config/alloy.toml")
//!     .with_env()
//!     .load()?;
//! ```

use std::path::{Path, PathBuf};

use figment::Figment;
#[cfg(any(feature = "yaml-config", feature = "toml-config"))]
use figment::providers::Format;
#[cfg(feature = "toml-config")]
use figment::providers::Toml;
#[cfg(feature = "yaml-config")]
use figment::providers::Yaml;
use figment::providers::{Env, Serialized};
use tracing::{debug, info, trace, warn};

use super::schema::AlloyConfig;
use crate::error::{ConfigError, ConfigResult};

/// Configuration profile for environment-specific settings.
#[derive(Debug, Clone, Default)]
pub enum Profile {
    /// Development profile (default).
    #[default]
    Development,
    /// Production profile.
    Production,
    /// Custom profile name.
    Custom(String),
}

impl Profile {
    /// Returns the profile name as a string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
            Self::Custom(name) => name,
        }
    }

    /// Creates a profile from environment variable or defaults to Development.
    pub fn from_env() -> Self {
        std::env::var("ALLOY_PROFILE")
            .map(|p| match p.to_lowercase().as_str() {
                "production" | "prod" => Self::Production,
                "development" | "dev" => Self::Development,
                other => Self::Custom(other.to_string()),
            })
            .unwrap_or_default()
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration loader with figment-based multi-source support.
///
/// # Example
///
/// ```rust,ignore
/// let config = ConfigLoader::new()
///     .file("alloy.yaml")
///     .with_env()
///     .load()?;
/// ```
pub struct ConfigLoader {
    /// Configuration profile.
    profile: Profile,
    /// Search paths for configuration files.
    search_paths: Vec<PathBuf>,
    /// Whether to load environment variables.
    load_env: bool,
    /// Specific config file to load (overrides search).
    config_file: Option<PathBuf>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    /// Creates a new configuration loader with defaults.
    pub fn new() -> Self {
        Self {
            profile: Profile::from_env(),
            search_paths: Vec::new(),
            load_env: true,
            config_file: None,
        }
    }

    /// Sets the configuration profile.
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        let p = profile.into();
        self.profile = match p.to_lowercase().as_str() {
            "production" | "prod" => Profile::Production,
            "development" | "dev" => Profile::Development,
            _ => Profile::Custom(p),
        };
        self
    }

    /// Adds a search path for configuration files.
    pub fn search_path(mut self, path: impl AsRef<Path>) -> Self {
        self.search_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds current directory to search paths.
    pub fn with_current_dir(self) -> Self {
        if let Ok(cwd) = std::env::current_dir() {
            self.search_path(cwd)
        } else {
            self
        }
    }

    /// Sets a specific configuration file to load.
    pub fn file(mut self, path: impl AsRef<Path>) -> Self {
        self.config_file = Some(path.as_ref().to_path_buf());
        self
    }

    /// Enables loading environment variables (default: true).
    pub fn with_env(mut self) -> Self {
        self.load_env = true;
        self
    }

    /// Disables loading environment variables.
    pub fn without_env(mut self) -> Self {
        self.load_env = false;
        self
    }

    /// Loads and returns the configuration.
    pub fn load(self) -> ConfigResult<AlloyConfig> {
        let profile = self.profile.clone();
        let figment = self.build_figment()?;

        let config: AlloyConfig = figment.extract().map_err(|e| {
            ConfigError::ParseError(format!("Failed to extract configuration: {e}"))
        })?;

        debug!(
            profile = %profile,
            logging_level = %config.logging.level,
            "Configuration loaded successfully"
        );

        Ok(config)
    }

    /// Builds the figment instance with all sources.
    fn build_figment(self) -> ConfigResult<Figment> {
        // Start with defaults
        let mut figment = Figment::from(Serialized::defaults(AlloyConfig::default()));

        // Load config files
        if let Some(path) = self.config_file {
            // Load specific file
            if path.exists() {
                info!(path = %path.display(), "Loading configuration file");
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                figment = match ext {
                    #[cfg(feature = "toml-config")]
                    "toml" => figment.merge(Toml::file(path)),
                    #[cfg(feature = "yaml-config")]
                    "yaml" | "yml" => figment.merge(Yaml::file(path)),
                    _ => {
                        return Err(ConfigError::ParseError(format!(
                            "Unsupported or disabled configuration file format: .{ext}"
                        )));
                    }
                }
            } else {
                return Err(ConfigError::FileNotFound(path.clone()));
            }
        } else {
            // Search for config files
            figment = self.load_config_files(figment);
        }

        // Load environment variables
        if self.load_env {
            trace!("Loading environment variables with ALLOY_ prefix");
            figment = figment.merge(Env::prefixed("ALLOY_").split("__"));
        }

        Ok(figment)
    }

    /// Common search logic for a single file format.
    ///
    /// Iterates `search_paths × base_names`, tries a profile-specific variant first, then the
    /// base file. Returns `(figment, true)` as soon as a base file is found (early return), or
    /// `(figment, false)` if nothing was located.
    #[cfg(any(feature = "toml-config", feature = "yaml-config"))]
    fn load_format_files<F>(&self, mut figment: Figment, exts: &[&str]) -> (Figment, bool)
    where
        F: Format,
    {
        let mut found = false;
        for search_path in &self.search_paths {
            for ext in exts {
                // Profile-specific: e.g. alloy.production.toml
                let profile_name = format!("alloy.{}.{}", self.profile.as_str(), ext);
                let profile_path = search_path.join(&profile_name);
                if profile_path.exists() {
                    debug!(path = %profile_path.display(), "Loading profile-specific config");
                    figment = figment.merge(F::file(profile_path));
                    found = true;
                }

                // Base file
                let base_name = format!("alloy.{ext}");
                let base_path = search_path.join(&base_name);
                if base_path.exists() {
                    info!(path = %base_path.display(), "Loading configuration file");
                    figment = figment.merge(F::file(base_path));
                    found = true;
                }
            }
        }
        (figment, found)
    }

    /// Searches for and loads configuration files from search paths.
    ///
    /// Which file formats are attempted is controlled by the `toml-config` and `yaml-config`
    /// feature flags.  Each enabled format is searched independently.
    #[allow(unused_variables)]
    #[allow(unused_mut)]
    fn load_config_files(&self, mut figment: Figment) -> Figment {
        let mut found = false;

        #[cfg(feature = "toml-config")]
        {
            let (f, ok) = self.load_format_files::<Toml>(figment, &["toml"]);
            figment = f;
            found |= ok;
        }

        #[cfg(feature = "yaml-config")]
        {
            let (f, ok) = self.load_format_files::<Yaml>(figment, &["yaml", "yml"]);
            figment = f;
            found |= ok;
        }

        if !found {
            warn!("No configuration file found, using defaults");
        }
        figment
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConfigLoader::new().without_env().load().unwrap();

        assert_eq!(config.logging.level.as_str(), "info");
    }

    #[test]
    fn test_profile_from_env() {
        // SAFETY: This test is single-threaded and we clean up immediately after
        unsafe {
            std::env::set_var("ALLOY_PROFILE", "production");
        }
        let profile = Profile::from_env();
        assert!(matches!(profile, Profile::Production));
        unsafe {
            std::env::remove_var("ALLOY_PROFILE");
        }
    }
}
