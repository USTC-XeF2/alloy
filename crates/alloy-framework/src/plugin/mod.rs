//! Plugin system for the Alloy framework.
//!
//! # Architecture
//!
//! Plugins are the primary unit of event handling.  Each plugin is represented
//! by a [`Plugin`] instance — a concrete struct that bundles:
//!
//! - An ordered list of **handler services** (tower services).
//! - Optional **lifecycle hooks** (`on_load`, `on_unload`).
//! - Optional **service-provider** metadata for inter-plugin dependency ordering.
//!
//! A [`PluginDescriptor`] is the *static, `Copy` handle* to a plugin — it carries
//! only metadata and a factory function pointer.  The runtime calls
//! [`PluginDescriptor::instantiate`] to create the live [`Plugin`].
//!
//! # Quick start
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//!
//! async fn echo(event: EventContext<MessageEvent>) -> anyhow::Result<String> {
//!     Ok(event.get_plain_text().to_string())
//! }
//!
//! pub static ECHO: PluginDescriptor = define_plugin! {
//!     name: "echo",
//!     handlers: [on_message().handler(echo)],
//! };
//! ```
//!
//! # Configuration
//!
//! Plugin configuration is loaded transparently from `alloy.yaml` under
//! `plugins.<name>`.  The runtime injects the raw JSON section into every
//! [`AlloyContext`] before the handler chain runs.  Handlers opt in by
//! declaring a [`PluginConfig<T>`] parameter:
//!
//! ```rust,ignore
//! #[derive(serde::Deserialize, Default)]
//! struct EchoConfig { prefix: String }
//!
//! async fn echo_handler(
//!     event: EventContext<MessageEvent>,
//!     cfg:   PluginConfig<EchoConfig>,  // auto-extracted; falls back to Default
//! ) -> anyhow::Result<String> {
//!     Ok(format!("{} {}", cfg.prefix, event.get_plain_text()))
//! }
//!
//! pub static ECHO: PluginDescriptor = define_plugin! {
//!     name: "echo",
//!     handlers: [on_message().handler(echo_handler)],
//! };
//! ```
//!
//! YAML:
//! ```yaml
//! plugins:
//!   echo:
//!     prefix: "[Bot]"
//! ```
//!
//! In `on_load`, the plugin's raw JSON section is available as `config_json`:
//!
//! ```rust,ignore
//! pub static MY: PluginDescriptor = define_plugin! {
//!     name: "my",
//!     provides: {
//!         "my.service": MyService,  // MyService::init(&config_json) called automatically
//!     },
//!     handlers: [],
//! };
//! ```
//!
//! # Inter-plugin services
//!
//! ```rust,ignore
//! pub static MY_PLUGIN: PluginDescriptor = define_plugin! {
//!     name: "my_plugin",
//!     depends_on: ["alloy.storage"],
//!     handlers: [on_message().handler(handler)],
//!     on_load: async {
//!         // Services from `depends_on` are available here
//!         let storage: Arc<StorageService> = services.get("alloy.storage").unwrap();
//!         info!("data dir: {}", storage.data_dir().display());
//!     },
//! };
//! ```

// ─── Submodules ──────────────────────────────────────────────────────────────
pub mod config;
pub mod core;
pub mod descriptor;
pub mod macros;
pub mod registry;
pub mod service_ref;

#[cfg(feature = "builtin")]
pub mod builtin;

// ─── Re-exports from submodules ──────────────────────────────────────────────
pub use config::PluginConfig;
pub use core::{Plugin, PluginMetadata, PluginType, ServiceEntry};
pub use descriptor::{ALLOY_PLUGIN_API_VERSION, PluginDescriptor};
pub use registry::PluginService;
pub use service_ref::ServiceRef;

// ─── Macro-internal re-export (needed by define_plugin! at call sites) ───────
#[doc(hidden)]
pub use futures::future::BoxFuture as __BoxFuture;
#[doc(hidden)]
pub use serde_json::Value as __JsonValue;
#[doc(hidden)]
pub use tower::util::BoxCloneSyncService as __BoxCloneSyncService;
