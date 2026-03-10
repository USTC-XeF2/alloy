//! Configuration for the command parsing system.

use serde::{Deserialize, Serialize};

/// Configuration for the command system.
///
/// Stored in [`BaseContext`](crate::context::BaseContext) and used by
/// [`CommandService`](super::layer::CommandService) to determine the default
/// command prefix when the service does not specify one explicitly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandConfig {
    /// The prefix string that must appear before a command name to be
    /// recognised (default: `"/"`).
    ///
    /// ```toml
    /// [command]
    /// default_start_tag = "!"
    /// ```
    pub default_start_tag: String,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            default_start_tag: "/".to_string(),
        }
    }
}
