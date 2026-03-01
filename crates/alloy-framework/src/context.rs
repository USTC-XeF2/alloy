//! Context and extractor system for the Alloy framework.
//!
//! This module provides three context types that together model how an event
//! is processed across multiple plugins:
//!
//! - [`BaseContext`] — the **shared** base for one dispatch cycle.  A single
//!   `Arc<BaseContext>` is created per incoming event and passed to every
//!   plugin.  It holds the event, the bot, and the propagation flag.
//!
//! - [`PluginContext`] — **plugin-specific** data attached per-plugin.
//!   Each plugin gets its own isolated state storage, config section, and
//!   access to declared services. State is not shared between plugins.
//!
//! - [`AlloyContext`] — the full context handed to handlers, combining an
//!   `Arc<BaseContext>` with a `PluginContext`.  Calling
//!   [`stop_propagation`](AlloyContext::stop_propagation) on any plugin's
//!   `AlloyContext` writes through to the shared base, stopping the chain
//!   for all subsequent plugins. Each plugin's state is completely isolated.

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, RwLock};
use serde_json::Value;

use crate::error::{ExtractError, ExtractResult};
use alloy_core::{BoxedBot, BoxedEvent};

/// Type alias for the heterogeneous service map values stored in the global registry.
///
/// The inner `dyn Any` is actually an `Arc<dyn ServiceTrait>` upcast to `Any` by the
/// plugin's service factory.  Consumers downcast it back to `Arc<dyn ServiceTrait>`
/// to call methods on the trait object.
pub type ServiceArc = Arc<dyn Any + Send + Sync>;

/// Maps `TypeId` → `(String ID, ServiceArc)` for fast O(1) service lookups by type.
/// Used as the primary service registry since most queries happen by TypeId.
/// The String ID is preserved for logging and debugging purposes.
pub type ServiceMap = HashMap<TypeId, (String, ServiceArc)>;

// =============================================================================
// BaseContext — shared base, one per dispatch cycle
// =============================================================================

/// The shared base context for a single event dispatch cycle.
///
/// One `BaseContext` is created per incoming event and wrapped in an `Arc`
/// that is cloned into every [`AlloyContext`] for that event.  This means:
///
/// - Stopping propagation in one plugin is immediately visible to the dispatch
///   loop and to all subsequent plugins.
/// - The event and bot are accessed without copying.
/// - Each plugin has its own isolated state through [`PluginContext`].
pub struct BaseContext {
    event: BoxedEvent,
    bot: BoxedBot,
    /// Cleared by any handler that calls [`AlloyContext::stop_propagation`].
    is_propagating: AtomicBool,
}

impl BaseContext {
    /// Creates a new shared event context.
    pub(crate) fn new(event: BoxedEvent, bot: BoxedBot) -> Self {
        Self {
            event,
            bot,
            is_propagating: AtomicBool::new(true),
        }
    }

    /// Returns `true` if the event is still propagating.
    pub(crate) fn is_propagating(&self) -> bool {
        self.is_propagating.load(Ordering::SeqCst)
    }

    pub(crate) fn stop_propagation(&self) {
        self.is_propagating.store(false, Ordering::SeqCst);
    }
}

impl std::fmt::Debug for BaseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseContext")
            .field("event", &self.event)
            .field("is_propagating", &self.is_propagating())
            .finish_non_exhaustive()
    }
}

// =============================================================================
// PluginContext — per-plugin data, one per plugin per dispatch
// =============================================================================

/// Plugin-specific data carried alongside the shared [`BaseContext`].
///
/// Every plugin gets its own `PluginContext` for each event dispatch.
/// This context includes:
/// - The plugin's name
/// - The plugin's config section from `alloy.yaml`
/// - Reference to the global service map for dynamic service access
///
/// This is intentionally a separate struct so that each plugin has its own:
/// - Config and declarations (via these fields)
/// - Guarantees about data isolation during event processing
/// - Dynamic service access via the global service map
#[derive(Debug)]
pub struct PluginContext {
    /// The name of the plugin.
    name: String,
    /// The plugin's config section from `alloy.yaml` (or an empty object).
    config: Arc<Value>,
    /// Service IDs this plugin declared (via `provides` or `depends_on`).
    /// Used to check if a service lookup should be allowed.
    service_ids: HashSet<String>,
    /// Reference to the global service map managed by PluginManager.
    /// Services are looked up dynamically from here, not stored locally.
    all_services: Arc<RwLock<ServiceMap>>,
}

impl PluginContext {
    /// Creates a new `PluginContext` with the given plugin name, config, and service declarations.
    pub(crate) fn new(
        name: String,
        config: Arc<Value>,
        service_ids: HashSet<String>,
        all_services: Arc<RwLock<ServiceMap>>,
    ) -> Self {
        Self {
            name,
            config,
            service_ids,
            all_services,
        }
    }

    /// Returns the name of the plugin.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Deserialise the plugin config section into `T`.
    ///
    /// Returns `Err` if the config is missing required fields or has the wrong
    /// shape; use `#[serde(default)]` on the struct to make all fields optional.
    pub fn get_config<T>(&self) -> serde_json::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        T::deserialize(self.config.as_ref())
    }

    /// Looks up a service by its trait-object type.
    ///
    /// Returns `None` if the service of type `T` was not declared by this
    /// plugin (via `provides` or `depends_on`) or if its provider plugin
    /// failed to load.  For ergonomic handler injection prefer
    /// [`ServiceRef<dyn YourTrait>`](crate::plugin::ServiceRef).
    pub fn get_service<T: ?Sized + 'static>(&self) -> Option<Arc<T>> {
        let services = self.all_services.read();
        if let Some((id, arc)) = services.get(&TypeId::of::<T>())
            && self.service_ids.contains(id)
        {
            return arc.downcast_ref::<Arc<T>>().map(Arc::clone);
        }
        None
    }
}

// =============================================================================
// AlloyContext — full context, handed to handlers
// =============================================================================

/// The full context object passed to handlers during event processing.
///
/// `AlloyContext` composes the **shared** [`BaseContext`] (base) with
/// **plugin-specific** [`PluginContext`] data.  Each plugin gets:
///
/// - **Isolated state**: Via `set_state`, `get_state`, etc. — each plugin's
///   state is completely isolated and not visible to other plugins.
/// - **Shared propagation**: Calling [`stop_propagation`](Self::stop_propagation)
///   prevents subsequent plugins from running.
/// - **Shared event/bot**: Access to the event and bot without copying.
///
/// # Example
///
/// ```rust,ignore
/// async fn handle(ctx: Arc<AlloyContext>) {
///     println!("event: {:?}", ctx.event());
///     ctx.set_state("my_data".to_string());  // isolated to this plugin
///     ctx.stop_propagation();                // no further plugins will run
///     ctx.bot().send(...).await.ok();
/// }
/// ```
#[derive(Debug)]
pub struct AlloyContext {
    base: Arc<BaseContext>,
    plugin: Arc<PluginContext>,
    /// Per-plugin isolated state storage for this event dispatch.
    /// Each plugin gets its own independent state that is not shared.
    state: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl AlloyContext {
    /// Creates a new `AlloyContext` from a shared base and plugin-specific data.
    pub(crate) fn new(base: Arc<BaseContext>, plugin: Arc<PluginContext>) -> Self {
        Self {
            base,
            plugin,
            state: Mutex::new(HashMap::new()),
        }
    }

    // ─── Shared base delegation ───────────────────────────────────────────────

    /// Returns a reference to the underlying boxed event.
    pub fn event(&self) -> &BoxedEvent {
        &self.base.event
    }

    /// Returns a reference to the bot.
    pub fn bot(&self) -> &BoxedBot {
        &self.base.bot
    }

    /// Returns a reference to the plugin-specific context data.
    pub fn plugin(&self) -> &PluginContext {
        &self.plugin
    }

    /// Looks up a service by its trait-object type, returning an error if not found.
    pub fn require_service<T: ?Sized + 'static>(&self) -> ExtractResult<Arc<T>> {
        self.plugin
            .get_service::<T>()
            .ok_or(ExtractError::ServiceNotFound(std::any::type_name::<T>()))
    }

    /// Stops propagation of this event to subsequent plugins.
    ///
    /// Writes through to the shared base context; the dispatch loop checks
    /// `is_propagating()` before handing the event to each next plugin.
    pub fn stop_propagation(&self) {
        self.base.stop_propagation();
    }

    /// Returns `true` if the event is still propagating.
    pub fn is_propagating(&self) -> bool {
        self.base.is_propagating()
    }

    /// Stores a value in this plugin's isolated state map.
    ///
    /// Each plugin has its own isolated state that is not visible to other plugins.
    /// Only one value per type can be stored; subsequent calls overwrite.
    pub fn set_state<T: Send + Sync + 'static>(&self, value: T) {
        self.state.lock().insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieves a cloned value from this plugin's isolated state map.
    pub fn get_state<T: Clone + 'static>(&self) -> Option<T> {
        self.state
            .lock()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Returns `true` if a value of type `T` exists in this plugin's state.
    pub fn has_state<T: 'static>(&self) -> bool {
        self.state.lock().contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns a value from this plugin's state.
    pub fn take_state<T: 'static>(&self) -> Option<T> {
        self.state
            .lock()
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }
}
