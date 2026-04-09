//! Context and extractor system for the Alloy framework.
//!
//! This module provides three context types that together model how an event
//! is processed across multiple plugins:
//!
//! - [`EventContext`] — the **shared** base for one dispatch cycle.  A single
//!   `Arc<EventContext>` is created per incoming event and passed to every
//!   plugin.  It holds the event, the bot, and the propagation flag.
//!
//! - [`PluginContext`] — **plugin-specific** data attached per-plugin.
//!   Each plugin gets its own isolated state storage, config section, and
//!   access to declared services. State is not shared between plugins.
//!
//! - [`HandlerContext`] — the full context handed to handlers, combining an
//!   `Arc<EventContext>` with a `PluginContext`.  Calling
//!   [`stop_propagation`](HandlerContext::stop_propagation) on any plugin's
//!   `HandlerContext` writes through to the shared base, stopping the chain
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
// State — isolated state storage system
// =============================================================================

/// An isolated state storage container for both per-dispatch handler state
/// and persistent per-plugin state.
///
/// `State` provides type-indexed storage for arbitrary values, allowing different
/// contexts to maintain their own independent state. Each value is stored by its
/// type ID, so only one value per type can be stored at a time.
#[derive(Debug)]
pub struct State {
    /// Per-type isolated state storage.
    data: Mutex<HashMap<TypeId, Box<dyn Any + Send>>>,
}

impl State {
    /// Creates a new empty `State`.
    pub(crate) fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    /// Stores a value in the state map.
    ///
    /// Only one value per type can be stored; subsequent calls overwrite.
    pub fn set<T: Send + 'static>(&self, value: T) {
        self.data.lock().insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieves a cloned value from the state map.
    pub fn get<T: Clone + 'static>(&self) -> Option<T> {
        self.data
            .lock()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Returns `true` if a value of type `T` exists in the state.
    pub fn has<T: 'static>(&self) -> bool {
        self.data.lock().contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns a value from the state map.
    pub fn take<T: 'static>(&self) -> Option<T> {
        self.data
            .lock()
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }
}

/// Runtime-scoped command context shared across plugin and event contexts.
#[cfg(feature = "command")]
pub(crate) struct CommandContext {
    pub config: crate::command::CommandConfig,
    pub help_provider: Mutex<HashMap<String, Arc<dyn crate::command::HelpProvider>>>,
}

// =============================================================================
// EventContext — shared base, one per dispatch cycle
// =============================================================================

/// The shared base context for a single event dispatch cycle.
///
/// One `EventContext` is created per incoming event and wrapped in an `Arc`
/// that is cloned into every [`HandlerContext`] for that event.  This means:
///
/// - Stopping propagation in one plugin is immediately visible to the dispatch
///   loop and to all subsequent plugins.
/// - The event and bot are accessed without copying.
/// - Each plugin has its own isolated state through [`PluginContext`].
pub struct EventContext {
    event: BoxedEvent,
    bot: BoxedBot,
    /// Cleared by any handler that calls [`HandlerContext::stop_propagation`].
    is_propagating: AtomicBool,
    /// Runtime-scoped command context.
    #[cfg(feature = "command")]
    command: Arc<crate::context::CommandContext>,
}

impl EventContext {
    /// Creates a new shared event context.
    pub(crate) fn new(
        event: BoxedEvent,
        bot: BoxedBot,
        #[cfg(feature = "command")] command: Arc<crate::context::CommandContext>,
    ) -> Self {
        Self {
            event,
            bot,
            is_propagating: AtomicBool::new(true),
            #[cfg(feature = "command")]
            command,
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

impl std::fmt::Debug for EventContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventContext")
            .field("event", &self.event)
            .field("is_propagating", &self.is_propagating())
            .finish_non_exhaustive()
    }
}

// =============================================================================
// PluginContext — per-plugin data, one per plugin per dispatch
// =============================================================================

/// Plugin-specific data carried alongside the shared [`EventContext`].
///
/// Every plugin gets its own `PluginContext` for each event dispatch.
/// This context includes:
/// - The plugin's name
/// - The plugin's config section from `alloy.toml`
/// - Reference to the global service map for dynamic service access
/// - A per-plugin isolated state storage
///
/// This is intentionally a separate struct so that each plugin has its own:
/// - Config and declarations (via these fields)
/// - Guarantees about data isolation during event processing
/// - Persistent state storage that persists across all event dispatches
/// - Dynamic service access via the global service map
#[derive(Debug)]
pub struct PluginContext {
    /// The name of the plugin.
    name: String,
    /// The plugin's config section from `alloy.toml` (or an empty object).
    config: Arc<Value>,
    /// Service IDs this plugin declared (via `provides` or `depends_on`).
    /// Used to check if a service lookup should be allowed.
    service_ids: HashSet<String>,
    /// Reference to the global service map managed by PluginManager.
    /// Services are looked up dynamically from here, not stored locally.
    all_services: Arc<RwLock<ServiceMap>>,
    /// Per-plugin persistent state storage.
    /// This state persists across all event dispatches for this plugin instance.
    state: State,
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
            state: State::new(),
        }
    }

    /// Returns the name of the plugin.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Deserialize the plugin config section into `T`.
    ///
    /// Returns `Err` if the config is missing required fields or has the wrong
    /// shape; use `#[serde(default)]` on the struct to make all fields optional.
    pub fn config<T>(&self) -> serde_json::Result<T>
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
    pub fn service<T: ?Sized + 'static>(&self) -> Option<Arc<T>> {
        let services = self.all_services.read();
        if let Some((id, arc)) = services.get(&TypeId::of::<T>())
            && self.service_ids.contains(id)
        {
            return arc.downcast_ref::<Arc<T>>().map(Arc::clone);
        }
        None
    }

    /// Returns a reference to the plugin's persistent state.
    pub fn state(&self) -> &State {
        &self.state
    }
}

// =============================================================================
// HandlerContext — full context, handed to handlers
// =============================================================================

/// The full context object passed to handlers during event processing.
///
/// `HandlerContext` composes the **shared** [`EventContext`] (base) with
/// **plugin-specific** [`PluginContext`] data.  Each plugin gets:
///
/// - **Handler state**: Via `set_state`, `get_state`, etc. — per-dispatch
///   isolated state that is completely isolated and not visible to other plugins.
/// - **Shared propagation**: Calling [`stop_propagation`](Self::stop_propagation)
///   prevents subsequent plugins from running.
/// - **Shared event/bot**: Access to the event and bot without copying.
/// - **Access to plugin persistent state**: Via `plugin().state()`
///
/// # Example
///
/// ```rust,ignore
/// async fn handle(ctx: &HandlerContext) {
///     println!("event: {:?}", ctx.event());
///     ctx.set_state("my_data".to_string());  // handler state, isolated to this dispatch
///     ctx.plugin().state().set_state(42);    // plugin persistent state
///     ctx.stop_propagation();                // no further plugins will run
///     ctx.bot().send(...).await.ok();
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HandlerContext {
    base: Arc<EventContext>,
    plugin: Arc<PluginContext>,
    /// Per-plugin isolated state storage for this event dispatch.
    /// Each plugin gets its own independent state that is not shared.
    state: Arc<State>,
}

impl HandlerContext {
    /// Creates a new `HandlerContext` from a shared base and plugin-specific data.
    pub(crate) fn new(base: Arc<EventContext>, plugin: Arc<PluginContext>) -> Self {
        Self {
            base,
            plugin,
            state: Arc::new(State::new()),
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
            .service::<T>()
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

    pub fn state(&self) -> &State {
        &self.state
    }

    #[cfg(feature = "command")]
    pub(crate) fn command(&self) -> &crate::context::CommandContext {
        &self.base.command
    }
}
