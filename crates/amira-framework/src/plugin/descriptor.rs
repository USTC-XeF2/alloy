//! Plugin descriptor — the static, `Copy` handle to a plugin.

use std::any::TypeId;
use std::sync::Arc;

use futures::future::BoxFuture;
use tower::BoxError;
use tower::util::BoxCloneSyncService;

use crate::context::{HandlerContext, PluginContext, ServiceArc};

use super::context::PluginLoadContext;

type OnLoadFn =
    fn(PluginLoadContext) -> BoxFuture<'static, Result<(), Box<dyn std::fmt::Display + Send>>>;

type OnUnloadFn = fn() -> BoxFuture<'static, ()>;

// ─── DependsOnEntry ──────────────────────────────────────────────────────────────────────────────

/// A single dependency entry in a plugin's `depends_on` list.
#[repr(C)]
#[derive(Debug)]
pub struct DependsOnEntry {
    /// Service ID (the dependency name/identifier).
    pub name: &'static str,

    /// Whether this dependency is required (`true` if prefixed with `!` in macro).
    /// If `true`, the plugin will not load if this service is missing.
    /// If `false`, the plugin loads even if this service is unavailable.
    pub required: bool,
}

// ─── ServiceEntry ─────────────────────────────────────────────────────────────

/// One entry in a plugin's declared service map.
///
/// The [`PluginManager`] iterates these entries during `load_all` and calls
/// each factory to materialise and register the service in the global registry,
/// **after** the plugin's `on_load` hook succeeds.
///
/// [`PluginManager`]: crate::manager::PluginManager
#[derive(Debug)]
pub struct ServiceEntry {
    /// Registry ID — value of `<dyn ServiceTrait as ServiceMeta>::ID`.
    pub id: &'static str,
    /// `TypeId::of::<dyn ServiceTrait>()` — the key in the service registry.
    pub type_id: TypeId,
    /// Async factory: initialises the impl, upcasts to `Arc<dyn ServiceTrait>`.
    /// Returns `Ok(ServiceArc)` on success, or `Err(String)` on failure.
    pub factory: fn(Arc<PluginContext>) -> BoxFuture<'static, Result<ServiceArc, String>>,
}

pub(crate) type BoxedHandlerService = BoxCloneSyncService<HandlerContext, (), BoxError>;

// ─── PluginDescriptor ─────────────────────────────────────────────────────────

/// A static, `Copy` descriptor that stores all plugin metadata and factories.
///
/// Use the [`define_plugin!`] macro to create one, then [`Plugin::new`] to
/// instantiate the live plugin.
///
/// # Memory layout
///
/// `PluginDescriptor` is `#[repr(C)]`.  Fields **must not be reordered**.
#[repr(C)]
#[derive(Debug)]
pub struct PluginDescriptor {
    /// Human-readable plugin name (used in logs and as config lookup key).
    pub name: &'static str,

    /// Semver version string (defaults to `CARGO_PKG_VERSION`).
    pub version: &'static str,

    /// Description — doc comment if present, otherwise `CARGO_PKG_DESCRIPTION`.
    pub desc: &'static str,

    /// Service entries registered into the global service map during load.
    pub provides: &'static [ServiceEntry],

    /// All dependencies declared in `depends_on: [...]`, with `required` flag indicating
    /// whether each is mandatory (`true` for marked with `!`) or optional (`false`).
    pub depends_on: &'static [DependsOnEntry],

    /// Called once at startup, before services are registered.
    pub on_load: Option<OnLoadFn>,
    /// Called once at shutdown.
    pub on_unload: Option<OnUnloadFn>,

    /// Factory that creates the handler services for this plugin.
    pub create_handlers: fn() -> Vec<BoxedHandlerService>,
}
