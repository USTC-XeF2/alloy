//! Built-in plugins shipped with the Alloy framework.
//!
//! These plugins are enabled by the `builtin` feature flag (on by default) and
//! provide common infrastructure services that most bots will need.
//!
//! | Plugin | Service ID | Description |
//! |--------|-----------|-------------|
//! | [`STORAGE_PLUGIN`] | `"alloy.storage"` | Structured filesystem storage |
//!
//! # Loading built-in plugins
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//! use alloy_framework::plugin::builtin::STORAGE_PLUGIN;
//!
//! runtime.register_plugin(STORAGE_PLUGIN).await;
//! ```
//!
//! Alternatively, call [`AlloyRuntime::load_builtin_plugins`] to load all
//! built-in plugins at once.

pub mod storage;
