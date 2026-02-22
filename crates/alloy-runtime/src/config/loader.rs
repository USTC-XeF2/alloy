//! Configuration loader using figment.
//!
//! This module provides a flexible configuration loading system that supports:
//!
//! - **Multiple sources**: YAML/TOML files, environment variables, programmatic defaults
//! - **Layered configuration**: Later sources override earlier ones
//! - **Profile support**: Development vs production configurations
//! - **Environment variable interpolation**: `${VAR}` or `${VAR:-default}`
//!
//! # Configuration Priority (lowest to highest)
//!
//! 1. Built-in defaults
//! 2. Profile-specific config file (`alloy.{profile}.yaml`)
//! 3. Main config file (`alloy.yaml`)
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
//!     .file("./config/alloy.yaml")
//!     .with_env()
//!     .load()?;
//! ```

use super::error::{ConfigError, ConfigResult};
use super::schema::AlloyConfig;
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Yaml};
use std::path::{Path, PathBuf};
use tracing::{debug, info, trace, warn};

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
    /// Base figment instance.
    figment: Figment,
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
            figment: Figment::new(),
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
    pub fn search_path<P: AsRef<Path>>(mut self, path: P) -> Self {
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

    /// Adds user config directory to search paths.
    pub fn with_user_config_dir(self) -> Self {
        if let Some(config_dir) = dirs::config_dir() {
            self.search_path(config_dir.join("alloy"))
        } else {
            self
        }
    }

    /// Sets a specific configuration file to load.
    pub fn file<P: AsRef<Path>>(mut self, path: P) -> Self {
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

    /// Merges additional configuration programmatically.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ConfigLoader::new()
    ///     .merge(AlloyConfig {
    ///         logging: LoggingConfig { level: LogLevel::Debug, ..Default::default() },
    ///         ..Default::default()
    ///     })
    ///     .load()?;
    /// ```
    pub fn merge(mut self, config: AlloyConfig) -> Self {
        self.figment = self.figment.merge(Serialized::defaults(config));
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
    fn build_figment(mut self) -> ConfigResult<Figment> {
        // Start with defaults
        let mut figment = Figment::from(Serialized::defaults(AlloyConfig::default()));

        // Merge user's pre-configured figment
        let user_figment = std::mem::take(&mut self.figment);
        figment = figment.merge(user_figment);

        // Load config files
        if let Some(path) = self.config_file {
            // Load specific file
            if path.exists() {
                info!(path = %path.display(), "Loading configuration file");
                figment = figment.merge(Yaml::file(path));
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
            figment = figment.merge(
                Env::prefixed("ALLOY_")
                    .split("__")
                    .map(|key| key.as_str().replace("__", ".").into()),
            );
        }

        Ok(figment)
    }

    /// Searches for and loads configuration files from search paths.
    fn load_config_files(&self, mut figment: Figment) -> Figment {
        const BASE_NAMES: &[&str] = &["alloy.yaml", "alloy.yml", "config.yaml", "config.yml"];

        let search_paths = if self.search_paths.is_empty() {
            // Default search paths
            let mut paths = Vec::new();
            if let Ok(cwd) = std::env::current_dir() {
                paths.push(cwd);
            }
            if let Some(config_dir) = dirs::config_dir() {
                paths.push(config_dir.join("alloy"));
            }
            paths
        } else {
            self.search_paths.clone()
        };

        // First, try to load profile-specific config
        let profile_suffix = format!(".{}", self.profile.as_str());
        for search_path in &search_paths {
            for base_name in BASE_NAMES {
                // Try profile-specific file (e.g., alloy.production.yaml)
                let profile_name = base_name.replace(".yaml", &format!("{profile_suffix}.yaml"));
                let profile_path = search_path.join(&profile_name);
                if profile_path.exists() {
                    debug!(path = %profile_path.display(), "Loading profile-specific config");
                    figment = figment.merge(Yaml::file(&profile_path));
                }

                // Try base config file
                let base_path = search_path.join(base_name);
                if base_path.exists() {
                    info!(path = %base_path.display(), "Loading configuration file");
                    figment = figment.merge(Yaml::file(&base_path));
                    return figment;
                }
            }
        }

        // No config file found, use defaults
        warn!("No configuration file found, using defaults");
        figment
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Loads configuration from default locations.
///
/// This is equivalent to `ConfigLoader::new().with_current_dir().load()`.
pub fn load_config() -> ConfigResult<AlloyConfig> {
    ConfigLoader::new().with_current_dir().load()
}

/// Loads configuration from a specific file.
pub fn load_config_from_file<P: AsRef<Path>>(path: P) -> ConfigResult<AlloyConfig> {
    ConfigLoader::new().file(path).load()
}

/// Loads configuration from a YAML string.
pub fn load_config_from_str(yaml: &str) -> ConfigResult<AlloyConfig> {
    let config: AlloyConfig =
        serde_yaml::from_str(yaml).map_err(|e| ConfigError::ParseError(e.to_string()))?;
    Ok(config)
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
        assert_eq!(config.runtime.shutdown_timeout_secs, 30);
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

    #[test]
    fn test_yaml_parsing() {
        let yaml = r#"
logging:
  level: debug
  format: json
runtime:
  shutdown_timeout_secs: 60
"#;

        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.logging.level.as_str(), "debug");
        assert_eq!(config.runtime.shutdown_timeout_secs, 60);
    }
}
