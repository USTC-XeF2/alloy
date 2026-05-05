//! Plugin system for the Amira framework.
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
//! metadata, lifecycle hooks, and the handler factory.  The runtime calls
//! [`Plugin::new`] to create the live [`Plugin`].
//!
//! # Quick start
//!
//! ```rust,ignore
//! use amira::prelude::*;
//!
//! async fn echo(event: Event<MessageEvent>) -> anyhow::Result<String> {
//!     Ok(event.plain_text())
//! }
//!
//! define_plugin! {
//!     name: "echo",
//!     handlers: [on_message().handler(echo)],
//! }
//! // Generates: pub static ECHO_PLUGIN: PluginDescriptor = { ... }
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
//!
//! impl ServiceInit for MyServiceImpl {
//!     async fn init(_ctx: PluginLoadContext) -> Self { MyServiceImpl }
//! }
//!
//! // 3. Register the service:
//! define_plugin! {
//!     name: "my_plugin",
//!     provides: {
//!         MyService: MyServiceImpl,
//!     },
//!     handlers: [],
//! }
//! // Generates: pub static MY_PLUGIN_PLUGIN: PluginDescriptor = { ... }
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
mod context;
mod core;
mod descriptor;
mod registry;

// ─── Re-exports from submodules ──────────────────────────────────────────────
pub use context::PluginLoadContext;
pub use core::Plugin;
pub use descriptor::{DependsOnEntry, PluginDescriptor, ServiceEntry};
pub use registry::{ServiceInit, ServiceMeta};

// ─── Macro-internal re-export (needed by define_plugin! at call sites) ───────
#[doc(hidden)]
pub use tower::util::BoxCloneSyncService as __BoxCloneSyncService;
