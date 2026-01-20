//! Configuration file loader.

use super::error::{ConfigError, ConfigResult};
use super::schema::AlloyConfig;
use super::validation::validate_config;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Configuration loader with support for multiple sources.
pub struct ConfigLoader {
    search_paths: Vec<PathBuf>,
}

impl ConfigLoader {
    /// Creates a new configuration loader.
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    /// Adds a search path for configuration files.
    pub fn add_search_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.search_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds the current directory to search paths.
    pub fn with_current_dir(self) -> Self {
        if let Ok(cwd) = std::env::current_dir() {
            self.add_search_path(cwd)
        } else {
            self
        }
    }

    /// Adds the user config directory to search paths.
    pub fn with_user_config_dir(self) -> Self {
        if let Some(config_dir) = dirs::config_dir() {
            self.add_search_path(config_dir.join("alloy"))
        } else {
            self
        }
    }

    /// Loads configuration from the first available source.
    pub fn load(&self) -> ConfigResult<AlloyConfig> {
        // Try to find and load configuration file
        if let Some(path) = self.find_config_file() {
            return self.load_from_file(&path);
        }

        // Return default config if no file found
        info!("No configuration file found, using defaults");
        Ok(AlloyConfig::default())
    }

    /// Loads configuration from a specific file.
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> ConfigResult<AlloyConfig> {
        let path = path.as_ref();
        info!("Loading configuration from: {}", path.display());

        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;
        let config = self.parse_yaml(&content)?;

        // Validate the configuration
        validate_config(&config)?;

        debug!(
            "Configuration loaded successfully with {} bot(s)",
            config.bots.len()
        );
        Ok(config)
    }

    /// Loads configuration from a YAML string.
    pub fn load_from_str(&self, yaml: &str) -> ConfigResult<AlloyConfig> {
        let config = self.parse_yaml(yaml)?;
        validate_config(&config)?;
        Ok(config)
    }

    /// Finds the first available configuration file.
    fn find_config_file(&self) -> Option<PathBuf> {
        const CONFIG_NAMES: &[&str] = &[
            "alloy.yaml",
            "alloy.yml",
            "config.yaml",
            "config.yml",
            ".alloy.yaml",
            ".alloy.yml",
        ];

        for search_path in &self.search_paths {
            for name in CONFIG_NAMES {
                let path = search_path.join(name);
                debug!("Checking for config file: {}", path.display());
                if path.exists() {
                    info!("Found configuration file: {}", path.display());
                    return Some(path);
                }
            }
        }

        None
    }

    /// Parses YAML content with environment variable expansion.
    fn parse_yaml(&self, content: &str) -> ConfigResult<AlloyConfig> {
        let expanded = self.expand_env_vars(content);
        serde_yaml::from_str(&expanded).map_err(ConfigError::from)
    }

    /// Expands environment variables in the format ${VAR_NAME} or ${VAR_NAME:-default}.
    fn expand_env_vars(&self, content: &str) -> String {
        let mut result = content.to_string();
        let re = regex_lite::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(:-([^}]*))?\}").unwrap();

        for cap in re.captures_iter(content) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_name = cap.get(1).unwrap().as_str();
            let default_value = cap.get(3).map(|m| m.as_str());

            let value = std::env::var(var_name)
                .ok()
                .or_else(|| default_value.map(String::from))
                .unwrap_or_default();

            result = result.replace(full_match, &value);
        }

        result
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new().with_current_dir().with_user_config_dir()
    }
}

/// Convenience function to load configuration with default settings.
pub fn load_config() -> ConfigResult<AlloyConfig> {
    ConfigLoader::default().load()
}

/// Convenience function to load configuration from a specific file.
pub fn load_config_from_file<P: AsRef<Path>>(path: P) -> ConfigResult<AlloyConfig> {
    ConfigLoader::new().load_from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_yaml() {
        let loader = ConfigLoader::new();
        let config = loader.load_from_str("").unwrap();
        assert!(config.bots.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
global:
  log_level: debug
"#;
        let loader = ConfigLoader::new();
        let config = loader.load_from_str(yaml).unwrap();
        assert_eq!(config.global.log_level, "debug");
    }

    #[test]
    fn test_parse_bot_config() {
        let yaml = r#"
bots:
  - id: test-bot
    adapter: onebot
    transport:
      type: ws-client
      url: ws://localhost:8080
"#;
        let loader = ConfigLoader::new();
        let config = loader.load_from_str(yaml).unwrap();
        assert_eq!(config.bots.len(), 1);
        assert_eq!(config.bots[0].id, "test-bot");
    }

    #[test]
    fn test_env_var_expansion() {
        // SAFETY: This test runs in single-threaded context
        unsafe { std::env::set_var("TEST_URL", "ws://test:8080") };
        let loader = ConfigLoader::new();

        let yaml = r#"
bots:
  - id: test-bot
    adapter: onebot
    transport:
      type: ws-client
      url: ${TEST_URL}
"#;
        let config = loader.load_from_str(yaml).unwrap();

        if let super::super::schema::TransportConfig::WsClient(ws_config) =
            &config.bots[0].transport
        {
            assert_eq!(ws_config.url, "ws://test:8080");
        } else {
            panic!("Expected WsClient transport");
        }

        // SAFETY: This test runs in single-threaded context
        unsafe { std::env::remove_var("TEST_URL") };
    }

    #[test]
    fn test_env_var_default_value() {
        let loader = ConfigLoader::new();

        let yaml = r#"
bots:
  - id: test-bot
    adapter: onebot
    transport:
      type: ws-client
      url: ${NONEXISTENT_VAR:-ws://default:8080}
"#;
        let config = loader.load_from_str(yaml).unwrap();

        if let super::super::schema::TransportConfig::WsClient(ws_config) =
            &config.bots[0].transport
        {
            assert_eq!(ws_config.url, "ws://default:8080");
        } else {
            panic!("Expected WsClient transport");
        }
    }
}
