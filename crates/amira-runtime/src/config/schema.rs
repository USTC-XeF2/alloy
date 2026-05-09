//! Configuration schema definitions using figment.
//!
//! This module defines the configuration structure for the Amira framework.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Root configuration structure for the Amira framework.
///
/// This struct is designed to be extended by adapters through the `adapters` field,
/// which holds adapter-specific configuration as dynamic values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AmiraConfig {
    /// Logging configuration.
    #[cfg(feature = "logging")]
    pub logging: crate::logging::LoggingConfig,

    /// Command system configuration.
    #[cfg(feature = "command")]
    pub command: amira_framework::command::CommandConfig,

    /// Adapter-specific configurations.
    ///
    /// Each adapter registers its own configuration schema.
    #[serde(default)]
    pub adapters: HashMap<String, serde_json::Value>,

    /// Plugin-specific configurations.
    ///
    /// Keyed by plugin name (must match the `name` field in the plugin descriptor).
    /// Each entry is deserialized into the plugin's declared `config_type` at load
    /// time and injected into every [`HandlerContext`] for that plugin run.
    ///
    /// ```toml
    /// [plugins.echo]
    /// prefix = "[Bot]"
    ///
    /// [plugins.'amira.storage']
    /// base_dir = "./bot_data"
    /// ```
    ///
    /// [`HandlerContext`]: amira_framework::context::HandlerContext
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,
}
