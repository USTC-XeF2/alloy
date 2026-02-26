//! Inter-plugin service traits.
//!
//! There are two complementary traits in this module:
//!
//! | Trait | Who implements it | Purpose |
//! |-------|-------------------|---------|
//! | [`ServiceMeta`] | `dyn YourServiceTrait` | Provides the string registry ID |
//! | [`ServiceInit`] | Concrete impl type | Async factory called at load time |
//!
//! # Design
//!
//! Services follow a **trait-object** pattern:
//!
//! 1. Define a **service trait** describing the public API:
//!
//! ```rust,ignore
//! pub trait MyService: Send + Sync + 'static {
//!     fn get_value(&self) -> String;
//! }
//!
//! // Give the trait a registry ID (implemented on the dyn version):
//! impl ServiceMeta for dyn MyService {
//!     const ID: &'static str = "my.service";
//! }
//! ```
//!
//! 2. Write a **concrete implementation** and make it initializable:
//!
//! ```rust,ignore
//! pub struct MyServiceImpl { value: String }
//!
//! impl MyService for MyServiceImpl { /* … */ }
//!
//! #[async_trait::async_trait]
//! impl ServiceInit for MyServiceImpl {
//!     async fn init(ctx: Arc<PluginLoadContext>) -> Self {
//!         let cfg = ctx.get_config::<MyConfig>().unwrap_or_default();
//!         MyServiceImpl { value: cfg.value }
//!     }
//! }
//! ```
//!
//! 3. Register with the new **map syntax** in [`define_plugin!`]:
//!
//! ```rust,ignore
//! pub static MY_PLUGIN: PluginDescriptor = define_plugin! {
//!     name: "my_plugin",
//!     provides: {
//!         dyn MyService: MyServiceImpl,
//!     },
//! };
//! ```
//!
//! 4. Consume in handlers via [`ServiceRef<dyn YourTrait>`]:
//!
//! ```rust,ignore
//! async fn my_handler(
//!     service: ServiceRef<dyn MyService>,
//! ) -> anyhow::Result<String> {
//!     let value = service.get_value();
//!     Ok(value)
//! }
//! ```

use std::sync::Arc;

use async_trait::async_trait;

use super::PluginLoadContext;

// ─── ServiceMeta ──────────────────────────────────────────────────────────────

/// Associates a static registry ID with a service *trait*.
///
/// Implement this for the **`dyn` version** of your service trait so the
/// framework can match `provides` entries with `depends_on` entries by ID.
pub trait ServiceMeta {
    /// Unique string identifier for this service interface.
    ///
    /// Used by [`define_plugin!`] to populate `provides` / `depends_on` lists
    /// and by [`PluginManager`] for dependency-order topological sorting.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    const ID: &'static str;
}

// ─── ServiceInit ──────────────────────────────────────────────────────────────

/// Async factory trait implemented by **concrete service implementation** types.
///
/// The [`define_plugin!`] macro calls `ServiceInit::init` once at plugin-load
/// time, then upcasts the result to the declared service trait object and
/// stores it in the global service registry.
#[async_trait]
pub trait ServiceInit: Send + Sync + Sized + 'static {
    /// Construct this service implementation from the plugin's load context.
    ///
    /// Called once at plugin load time.  May perform async I/O (e.g. creating
    /// directories, connecting to databases).  Use `ctx.get_config::<T>()` to
    /// deserialise a typed configuration struct.
    async fn init(ctx: Arc<PluginLoadContext>) -> Self;
}
