//! Inter-plugin service trait.
//!
//! The [`PluginService`] trait is implemented by types that can be registered
//! as inter-plugin services.  The [`PluginManager`](crate::manager::PluginManager)
//! manages the actual service map — plugins only need to implement this trait
//! and declare the type in `provides: [...]` inside [`define_plugin!`].
//!
//! # Basic usage
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//!
//! struct MyService { /* … */ }
//!
//! #[async_trait::async_trait]
//! impl PluginService for MyService {
//!     const ID: &'static str = "my_service";
//!     async fn init(config: &serde_json::Value) -> Self { MyService { /* … */ } }
//! }
//!
//! pub static MY_PLUGIN: PluginDescriptor = define_plugin! {
//!     name: "my_plugin",
//!     provides: [MyService],
//!     handlers: [],
//! };
//! ```

use std::any::Any;

use async_trait::async_trait;

// ─── PluginService trait ──────────────────────────────────────────────────────

/// Trait for types that can be registered as inter-plugin services.
///
/// Implementors declare how to construct themselves from the plugin's raw
/// config section by providing an async [`init`](PluginService::init) method.
/// The runtime calls this once during plugin load, wraps the result in
/// `Arc::new(…)`, and inserts it into the manager's service map —
/// all automatically, via the `provides` field in [`define_plugin!`].
#[async_trait]
pub trait PluginService: Any + Send + Sync {
    /// Static identifier for this service type.
    ///
    /// Used by [`define_plugin!`] and [`PluginManager`] to register and lookup
    /// services without needing external ID constants or configuration.
    /// Each implementation must define its own unique ID.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    const ID: &'static str;

    /// Construct this service from the plugin's raw config JSON section.
    ///
    /// Called once at plugin load time.  This method may be async, allowing
    /// for I/O operations (e.g. creating directories, connecting to databases).
    /// Use `serde_json::from_value` to deserialise a typed config struct;
    /// fall back to `Default` if absent.
    async fn init(config: &serde_json::Value) -> Self
    where
        Self: Sized;
}
