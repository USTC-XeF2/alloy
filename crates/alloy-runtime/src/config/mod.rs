//! Configuration module for Alloy runtime.
//!
//! This module provides a flexible, extensible configuration system built on top of
//! [figment](https://docs.rs/figment). It supports:
//!
//! - **Multi-source configuration**: TOML/YAML files, environment variables, programmatic defaults
//! - **Layered merging**: Later sources override earlier ones
//! - **Profile support**: Different configurations for development/production
//! - **Adapter extensibility**: Adapters can define their own configuration schemas
//!
//! # Environment Variable Override
//!
//! All configuration values can be overridden via environment variables:
//!
//! - `ALLOY_LOGGING__LEVEL=debug`
//! - `ALLOY_NETWORK__TIMEOUT_SECS=60`
//! - `ALLOY_ADAPTERS__ONEBOT__ACCESS_TOKEN=secret`

pub mod loader;
pub mod schema;

pub use loader::{ConfigLoader, Profile};
pub use schema::{AlloyConfig, LogFormat, LogLevel, LogOutput, LoggingConfig, SpanEventConfig};
