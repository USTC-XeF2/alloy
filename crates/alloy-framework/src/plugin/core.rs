use std::any::TypeId;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::future::BoxFuture;
use tower::BoxError;
use tower::util::BoxCloneSyncService;

use crate::context::{HandlerContext, PluginContext, ServiceArc};

/// Load-time plugin context that extends [`PluginContext`] with command registration.
///
/// This context is only used by plugin lifecycle hooks such as `on_load`.
#[derive(Clone)]
pub struct PluginLoadContext {
    plugin: Arc<PluginContext>,
    #[cfg(feature = "command")]
    command: Arc<crate::context::CommandContext>,
}

impl PluginLoadContext {
    pub(crate) fn new(
        plugin: Arc<PluginContext>,
        #[cfg(feature = "command")] command: Arc<crate::context::CommandContext>,
    ) -> Self {
        Self {
            plugin,
            #[cfg(feature = "command")]
            command,
        }
    }

    /// Registers this plugin's help provider into the runtime-scoped command context.
    #[cfg(feature = "command")]
    pub fn register_commands<F, T>(&self, provider: F)
    where
        F: crate::handler::FromCtxFn<T, Response = crate::command::CommandMap>,
        T: Send + Sync + 'static,
    {
        self.command.help_provider.write().insert(
            self.plugin.name().to_string(),
            Arc::new((provider, PhantomData)),
        );
    }
}

impl std::ops::Deref for PluginLoadContext {
    type Target = PluginContext;

    fn deref(&self) -> &Self::Target {
        &self.plugin
    }
}

/// Type of the async `on_load` function stored inside a [`Plugin`].
///
/// Must return `Ok(())` on success or `Err(BoxError)` on failure.
pub type OnLoadFn = fn(PluginLoadContext) -> BoxFuture<'static, Result<(), BoxError>>;

/// Type of the async `on_unload` function stored inside a [`Plugin`].
pub type OnUnloadFn = fn() -> BoxFuture<'static, ()>;

// ─── PluginType ───────────────────────────────────────────────────────────────

/// Describes what functional role a plugin plays.
///
/// The value is **auto-inferred** by the [`define_plugin!`] macro:
/// - Plugins that declare a non-empty `provides` list → [`PluginType::Service`].
/// - All other plugins → [`PluginType::Runtime`].
///
/// The inferred value can be overridden by setting `plugin_type` inside the
/// `metadata` block:
///
/// ```rust,ignore
/// define_plugin! {
///     name: "my_plugin",
///     metadata: { plugin_type: service },
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Plugin primarily registers shared services for other plugins to consume.
    Service,
    /// Plugin primarily handles events / has active runtime behaviour.
    Runtime,
}

// ─── PluginMetadata ───────────────────────────────────────────────────────────

/// Descriptive metadata attached to every plugin.
///
/// Populated automatically by the [`define_plugin!`] macro; values are derived
/// from build-environment constants and the optional `metadata` block.
///
/// # Defaults
///
/// | Field | Default |
/// |-------|---------|
/// | `version` | `CARGO_PKG_VERSION` of the crate that defined the plugin |
/// | `plugin_type` | auto-inferred from whether `provides` is non-empty |
/// | `desc` | `CARGO_PKG_DESCRIPTION` of the defining crate, or `""` |
/// | `full_desc` | `None` |
///
/// # Overriding via `define_plugin!`
///
/// ```rust,ignore
/// pub static MY: PluginDescriptor = define_plugin! {
///     name: "my_plugin",
///     metadata: {
///         version:     "2.0.0",
///         plugin_type: runtime,
///         desc:        "Short description.",
///         full_desc:   "Even longer description.",
///     },
///     handlers: [...],
/// };
/// ```
///
/// The `///` doc comment above `define_plugin!` (or above `name:`) is
/// captured as `full_desc` when `full_desc` is not set explicitly in the
/// `metadata` block.
#[repr(C)]
#[derive(Debug)]
pub struct PluginMetadata {
    /// Semver version string of the plugin.
    pub version: &'static str,
    /// Functional role of the plugin (service-oriented vs. runtime/event-driven).
    pub plugin_type: PluginType,
    /// One-line description shown in logs and registries.
    pub desc: &'static str,
    /// Optional long-form description.
    pub full_desc: Option<&'static str>,
}

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

type BoxedHandlerService = BoxCloneSyncService<HandlerContext, (), BoxError>;

// ─── Plugin ───────────────────────────────────────────────────────────────────

/// A live plugin instance bundling handlers, lifecycle hooks, and configuration.
///
/// Create via the [`define_plugin!`] macro.
///
/// # Concurrency
///
/// `Plugin` is `Send + Sync`.  Use interior mutability (e.g. `Arc<Mutex<T>>`)
/// for state that changes across events.
#[derive(Debug)]
pub struct Plugin {
    name: Cow<'static, str>,
    /// All dependencies declared in `depends_on: [...]`, with `required` flag indicating
    /// whether each is mandatory (`true` for marked with `!`) or optional (`false`).
    depends_on: Vec<DependsOnEntry>,
    handlers: Vec<BoxedHandlerService>,

    /// Service factories generated by the [`define_plugin!`] macro.
    ///
    /// The service IDs that this plugin *provides* are derived from these
    /// entries (`entry.id`), so there is no separate `provides` list —
    /// the two can never get out of sync.
    ///
    /// [`PluginManager`] iterates these during `load_all`, calls every
    /// factory, then registers the result via
    /// factory, then inserts the result into the global service map.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    service_entries: Vec<ServiceEntry>,

    on_load_fn: Option<OnLoadFn>,
    on_unload_fn: Option<OnUnloadFn>,

    /// Descriptive metadata for this plugin instance.
    metadata: PluginMetadata,
}

impl Plugin {
    /// Returns the plugin's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the plugin's metadata.
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Service IDs this plugin registers into the global registry.
    ///
    /// Derived on-demand from [`service_entries`](Self::service_entries)
    /// so the two can never get out of sync.
    pub fn provides(&self) -> Vec<&'static str> {
        self.service_entries.iter().map(|e| e.id).collect()
    }

    /// All dependencies declared in this plugin's `depends_on: [...]` list.
    pub fn depends_on(&self) -> &[DependsOnEntry] {
        &self.depends_on
    }

    /// Returns a slice of this plugin's handlers for the [`PluginManager`] to drive.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    pub(crate) fn handlers(&self) -> &[BoxedHandlerService] {
        &self.handlers
    }

    /// Service factory entries declared by this plugin.
    ///
    /// The [`PluginManager`] iterates these during `load_all` to materialise
    /// and register each service in the global registry **after** a successful
    /// call to [`on_load`](Self::on_load).
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    pub(crate) fn service_entries(&self) -> &[ServiceEntry] {
        &self.service_entries
    }

    /// Called once at startup, **before** services declared in `provides` are
    /// registered.
    ///
    /// Returns `Ok(())` when the plugin loaded successfully.  Returning `Err`
    /// causes [`PluginManager`] to mark the plugin as
    /// [`PluginLoadState::Failed`] and skip service registration entirely.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    /// [`PluginLoadState::Failed`]: crate::manager::PluginLoadState::Failed
    pub(crate) async fn on_load(&self, ctx: PluginLoadContext) -> Result<(), BoxError> {
        if let Some(f) = &self.on_load_fn {
            f(ctx).await
        } else {
            Ok(())
        }
    }

    /// Called once at shutdown.
    pub(crate) async fn on_unload(&self) {
        if let Some(f) = &self.on_unload_fn {
            f().await;
        }
    }
}

// ─── Internal constructor (used by define_plugin! macro) ─────────────────────

impl Plugin {
    /// Creates a `Plugin` directly.  Only called by the [`define_plugin!`] macro.
    #[doc(hidden)]
    pub fn __new(
        name: &'static str,
        depends_on: Vec<DependsOnEntry>,
        handlers: Vec<BoxedHandlerService>,
        service_entries: Vec<ServiceEntry>,
        on_load_fn: Option<OnLoadFn>,
        on_unload_fn: Option<OnUnloadFn>,
        metadata: PluginMetadata,
    ) -> Self {
        Plugin {
            name: Cow::Borrowed(name),
            depends_on,
            handlers,
            service_entries,
            on_load_fn,
            on_unload_fn,
            metadata,
        }
    }
}
