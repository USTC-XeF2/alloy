//! Context and extractor system for the Alloy framework.
//!
//! This module provides three context types that together model how an event
//! is processed across multiple plugins:
//!
//! - [`BaseContext`] — the **shared** base for one dispatch cycle.  A single
//!   `Arc<BaseContext>` is created per incoming event and passed to every
//!   plugin.  It holds the event, the bot, the propagation flag, and the
//!   cross-plugin state map.
//!
//! - [`PluginContext`] — **plugin-specific** data attached per-plugin.
//!   Currently holds only the plugin's config section, but serves as the
//!   extension point for future per-plugin fields.
//!
//! - [`AlloyContext`] — the full context handed to handlers, combining an
//!   `Arc<BaseContext>` with a `PluginContext`.  Calling
//!   [`stop_propagation`](AlloyContext::stop_propagation) on any plugin's
//!   `AlloyContext` writes through to the shared base, stopping the chain
//!   for all subsequent plugins.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use alloy_core::{BoxedBot, BoxedEvent};

/// A read-only snapshot of all registered inter-plugin services.
///
/// Keyed by `TypeId` so handlers can retrieve services by their concrete type.
/// Created by [`PluginManager`](crate::manager::PluginManager) once per
/// dispatch and shared (via `Arc`) across every [`AlloyContext`] for that event.
type ServiceSnapshot = HashMap<TypeId, Arc<dyn Any + Send + Sync>>;

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
/// - State written by one plugin via [`AlloyContext::set_state`] is readable
///   by later plugins via their own `AlloyContext`.
/// - The event and bot are accessed without copying.
pub struct BaseContext {
    event: BoxedEvent,
    bot: BoxedBot,
    /// Cleared by any handler that calls [`AlloyContext::stop_propagation`].
    is_propagating: AtomicBool,
    /// Type-keyed state shared across all plugins for this event.
    state: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl BaseContext {
    /// Creates a new shared event context.
    pub(crate) fn new(event: BoxedEvent, bot: BoxedBot) -> Self {
        Self {
            event,
            bot,
            is_propagating: AtomicBool::new(true),
            state: Mutex::new(HashMap::new()),
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
        let state_count = self.state.lock().map(|s| s.len()).unwrap_or(0);
        f.debug_struct("BaseContext")
            .field("event", &self.event)
            .field("is_propagating", &self.is_propagating())
            .field("state_entries", &state_count)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// PluginContext — per-plugin data, one per plugin per dispatch
// =============================================================================

/// Plugin-specific data carried alongside the shared [`BaseContext`].
///
/// Every plugin gets its own `PluginContext` for each event dispatch.
/// This is intentionally a plain struct rather than a field on [`AlloyContext`]
/// so that future per-plugin fields (e.g. per-plugin rate-limit state,
/// per-plugin metadata) have a clear home without polluting the shared base.
///
/// `services` is intentionally restricted to the services declared by this
/// plugin in its [`define_plugin!`] descriptor (`provides` + `depends_on`).
/// This prevents a plugin from reaching services it never declared a need for.
#[derive(Debug)]
pub struct PluginContext {
    /// The plugin's config section from `alloy.yaml` (or an empty object).
    config: Arc<Value>,
    /// Services accessible to this plugin — only those it declared.
    services: ServiceSnapshot,
}

impl PluginContext {
    /// Creates a new `PluginContext` with the given config and services.
    pub(crate) fn new(config: Arc<Value>, services: ServiceSnapshot) -> Self {
        Self { config, services }
    }
}

// =============================================================================
// AlloyContext — full context, handed to handlers
// =============================================================================

/// The full context object passed to handlers during event processing.
///
/// `AlloyContext` composes the **shared** [`BaseContext`] (base) with
/// **plugin-specific** [`PluginContext`] data.  All plugins handling the same
/// event share the same `Arc<BaseContext>`, so:
///
/// - Calling [`stop_propagation`](Self::stop_propagation) prevents subsequent
///   plugins from running.
/// - State written with [`set_state`](Self::set_state) is visible to plugins
///   processed later in the same dispatch cycle.
///
/// # Example
///
/// ```rust,ignore
/// async fn handle(ctx: Arc<AlloyContext>) {
///     println!("event: {:?}", ctx.event());
///     ctx.stop_propagation();           // no further plugins will run
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

    /// Stores a value in the shared cross-plugin state map.
    ///
    /// Only one value per type can be stored; subsequent calls overwrite.
    pub fn set_state<T: Send + Sync + 'static>(&self, value: T) {
        self.base
            .state
            .lock()
            .unwrap()
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieves a cloned value from the shared state map.
    pub fn get_state<T: Clone + 'static>(&self) -> Option<T> {
        self.base
            .state
            .lock()
            .unwrap()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Returns `true` if a value of type `T` exists in state.
    pub fn has_state<T: 'static>(&self) -> bool {
        self.base
            .state
            .lock()
            .unwrap()
            .contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns a value from state.
    pub fn take_state<T: 'static>(&self) -> Option<T> {
        self.base
            .state
            .lock()
            .unwrap()
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }

    // ─── Plugin-specific ──────────────────────────────────────────────────────

    /// Returns the plugin's config section from `alloy.yaml`.
    pub fn config(&self) -> &Arc<serde_json::Value> {
        &self.plugin.config
    }

    /// Returns a reference to the plugin-specific context.
    pub fn plugin_ctx(&self) -> &PluginContext {
        &self.plugin
    }
}
