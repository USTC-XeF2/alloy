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
//! # Service pattern
//!
//! Plugins can provide shared services via the `provides` map:
//!
//! ```rust,ignore
//! // 1. Define the service as a trait:
//! pub trait MyService: Send + Sync + 'static {
//!     fn do_thing(&self) -> String;
//! }
//! impl ServiceMeta for dyn MyService {
//!     const ID: &'static str = "my.service";
//! }
//!
//! // 2. Implement with a concrete type:
//! pub struct MyServiceImpl;
//! impl MyService for MyServiceImpl { fn do_thing(&self) -> String { "done".into() } }
//! #[async_trait::async_trait]
//! impl ServiceInit for MyServiceImpl {
//!     async fn init(_ctx: Arc<PluginLoadContext>) -> Self { MyServiceImpl }
//! }
//!
//! // 3. Register the service:
//! pub static MY: PluginDescriptor = define_plugin! {
//!     name: "my_plugin",
//!     provides: {
//!         MyService: MyServiceImpl,
//!     },
//!     handlers: [],
//! };
//! ```
//!
//! # Consuming services in handlers
//!
//! ```rust,ignore
//! async fn my_handler(
//!     svc: ServiceRef<dyn MyService>,
//! ) -> anyhow::Result<String> {
//!     Ok(svc.do_thing())
//! }
//! ```

// ─── Submodules ──────────────────────────────────────────────────────────────
pub mod config;
pub mod core;
pub mod descriptor;
pub mod macros;
pub mod registry;
pub mod service_ref;

#[cfg(feature = "builtin-plugins")]
pub mod builtin;

// ─── Re-exports from submodules ──────────────────────────────────────────────
pub use config::PluginConfig;
pub use core::{
    OnLoadFn, OnUnloadFn, Plugin, PluginLoadContext, PluginMetadata, PluginType, ServiceEntry,
};
pub use descriptor::{ALLOY_PLUGIN_API_VERSION, PluginDescriptor};
pub use registry::{ServiceInit, ServiceMeta};
pub use service_ref::ServiceRef;

// ─── Macro-internal re-export (needed by define_plugin! at call sites) ───────
#[doc(hidden)]
pub use tower::util::BoxCloneSyncService as __BoxCloneSyncService;
