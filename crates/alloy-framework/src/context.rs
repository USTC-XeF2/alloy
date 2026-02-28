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
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use alloy_core::{BoxedBot, BoxedEvent};

use crate::error::{ExtractError, ExtractResult};

/// Type alias for the heterogeneous service map values stored in the global registry.
///
/// The inner `dyn Any` is actually an `Arc<dyn ServiceTrait>` upcast to `Any` by the
/// plugin's service factory.  Consumers downcast it back to `Arc<dyn ServiceTrait>`
/// to call methods on the trait object.
pub type ServiceArc = Arc<dyn Any + Send + Sync>;

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
/// - The plugin's config section from `alloy.yaml`
/// - Services accessible to this plugin
/// - **Isolated state storage** unique to this plugin (not shared with other plugins)
///
/// This is intentionally a separate struct so that each plugin has its own:
/// - State space (via the `state` field)
/// - Config and services snapshot (via the other fields)
/// - Guarantees about data isolation during event processing
#[derive(Debug)]
pub struct PluginContext {
    /// The name of the plugin.
    name: String,
    /// The plugin's config section from `alloy.yaml` (or an empty object).
    config: Arc<Value>,
    /// Services accessible to this plugin — only those it declared.
    services: HashMap<TypeId, ServiceArc>,
    /// Per-plugin isolated state storage for this event dispatch.
    /// Each plugin gets its own independent state that is not shared.
    state: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl PluginContext {
    /// Creates a new `PluginContext` with the given plugin name, config, and services.
    pub(crate) fn new(
        name: &str,
        config: Arc<Value>,
        services: HashMap<TypeId, ServiceArc>,
    ) -> Self {
        Self {
            name: name.to_string(),
            config,
            services,
            state: Mutex::new(HashMap::new()),
        }
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
    plugin: PluginContext,
}

impl AlloyContext {
    /// Creates a new `AlloyContext` from a shared base and plugin-specific data.
    pub(crate) fn new(base: Arc<BaseContext>, plugin: PluginContext) -> Self {
        Self { base, plugin }
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

    /// Returns a clone of the bot `Arc`.
    pub fn bot_arc(&self) -> BoxedBot {
        self.base.bot.clone()
    }

    /// Looks up a service by its trait-object type.
    ///
    /// Returns `None` if the service of type `T` was not declared by this
    /// plugin (via `provides` or `depends_on`) or if its provider plugin
    /// failed to load.  For ergonomic handler injection prefer
    /// [`ServiceRef<dyn YourTrait>`](crate::plugin::ServiceRef).
    pub fn get_service<T: ?Sized + 'static>(&self) -> Option<Arc<T>> {
        self.plugin
            .services
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<Arc<T>>().map(Arc::clone))
    }

    pub fn require_service<T: ?Sized + 'static>(&self) -> ExtractResult<Arc<T>> {
        self.get_service::<T>()
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
        self.plugin
            .state
            .lock()
            .unwrap()
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieves a cloned value from this plugin's isolated state map.
    pub fn get_state<T: Clone + 'static>(&self) -> Option<T> {
        self.plugin
            .state
            .lock()
            .unwrap()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Returns `true` if a value of type `T` exists in this plugin's state.
    pub fn has_state<T: 'static>(&self) -> bool {
        self.plugin
            .state
            .lock()
            .unwrap()
            .contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns a value from this plugin's state.
    pub fn take_state<T: 'static>(&self) -> Option<T> {
        self.plugin
            .state
            .lock()
            .unwrap()
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }

    // ─── Plugin-specific ──────────────────────────────────────────────────────

    /// Returns the name of the currently executing plugin.
    pub fn get_plugin_name(&self) -> &str {
        &self.plugin.name
    }

    /// Returns the plugin's config section from `alloy.yaml`.
    pub fn get_config(&self) -> Arc<serde_json::Value> {
        self.plugin.config.clone()
    }
}
