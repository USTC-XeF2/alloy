//! Configuration module for Alloy runtime.
//!
//! This module provides a flexible, extensible configuration system built on top of
//! [figment](https://docs.rs/figment). It supports:
//!
//! - **Multi-source configuration**: YAML files, environment variables, programmatic defaults
//! - **Layered merging**: Later sources override earlier ones
//! - **Profile support**: Different configurations for development/production
//! - **Adapter extensibility**: Adapters can define their own configuration schemas
//!
//! # Configuration Structure
//!
//! ```yaml
//! # Logging configuration
//! logging:
//!   level: info          # trace, debug, info, warn, error
//!   format: pretty       # pretty, compact, json, full
//!   output: stdout       # stdout, stderr, file
//!
//! # Global network defaults
//! network:
//!   timeout_secs: 30
//!   retry:
//!     max_attempts: 3
//!     initial_delay_ms: 100
//!
//! # Runtime behavior
//! runtime:
//!   shutdown_timeout_secs: 30
//!   event_buffer_size: 1000
//!
//! # Adapter-specific configurations (extensible)
//! adapters:
//!   onebot:
//!     connection:
//!       type: ws-client
//!       url: ws://localhost:8080/onebot/v11/ws
//!     access_token: "your-token"
//! ```
//!
//! # Loading Configuration
//!
//! ```rust,ignore
//! use alloy_runtime::config::{ConfigLoader, AlloyConfig};
//!
//! // Simple loading
//! let config = load_config()?;
//!
//! // Custom loading
//! let config = ConfigLoader::new()
//!     .file("./config/alloy.yaml")
//!     .profile("production")
//!     .with_env()
//!     .load()?;
//! ```
//!
//! # Environment Variable Override
//!
//! All configuration values can be overridden via environment variables:
//!
//! - `ALLOY_LOGGING__LEVEL=debug`
//! - `ALLOY_NETWORK__TIMEOUT_SECS=60`
//! - `ALLOY_ADAPTERS__ONEBOT__ACCESS_TOKEN=secret`

pub mod error;
pub mod loader;
pub mod schema;

pub use error::{ConfigError, ConfigResult};
pub use loader::{ConfigLoader, Profile, load_config, load_config_from_file, load_config_from_str};
pub use schema::{
    AlloyConfig, ConnectionConfig, HttpClientConfig, HttpServerConfig, LogFormat, LogLevel,
    LogOutput, LoggingConfig, NetworkConfig, RetryConfig, RuntimeConfig, SpanEventConfig,
    WsClientConfig, WsServerConfig,
};
