//! Configuration module for Alloy runtime.
//!
//! This module provides YAML-based configuration loading and validation
//! for bot instances, transport settings, and global options.

pub mod error;
pub mod loader;
pub mod schema;
pub mod validation;

pub use error::{ConfigError, ConfigResult};
pub use loader::{ConfigLoader, load_config, load_config_from_file};
pub use schema::{
    AlloyConfig, BotConfig, GlobalConfig, HttpClientConfig, HttpServerConfig, RetryConfig,
    TransportConfig, WsClientConfig, WsServerConfig,
};
pub use validation::validate_config;
