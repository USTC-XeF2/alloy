//! Plugin lifecycle management and event dispatch.
//!
//! [`PluginManager`] is the central owner of all registered plugins. It:
//!
//! - Accepts [`PluginDescriptor`]s and instantiates them into live [`Plugin`]s
//!   with an initial state of [`PluginLoadState::Registered`].
//! - Drives plugin lifecycle (`on_load` / `on_unload`) in dependency order via
//!   [`load_all`](PluginManager::load_all) / [`unload_all`](PluginManager::unload_all).
//! - On `load_all`, checks that every declared dependency is satisfied;
//!   plugins with unmet dependencies are marked [`PluginLoadState::Failed`]
//!   and skipped — their services are never registered and their handlers are
//!   never invoked.
//! - Directly owns the **global service map** shared by all active plugins.
//!   Services are registered into it on load and removed from it on unload.
//!   During event dispatch each plugin receives a **restricted snapshot** of
//!   only the services it declared (via `provides` or `depends_on`) as part
//!   of its own [`PluginContext`](crate::context::PluginContext).
//! - Implements [`Dispatcher`]: on each incoming event it invokes all **active**
//!   plugins **sequentially** in registration order, sharing a single
//!   [`EventContext`](crate::context::EventContext).  Any plugin may call
//!   `stop_propagation` to short-circuit the remaining plugins.
//!
//! # Example
//!
//! ```rust,ignore
//! use amira_framework::manager::PluginManager;
//!
//! let manager = Arc::new(PluginManager::new(HashMap::new()));
//! manager.register_plugin(MY_PLUGIN);
//! manager.load_all().await;
//! // …later…
//! manager.unload_all().await;
//! ```

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use amira_core::bridge::Dispatcher;
use amira_core::{Bot, EventRoot};
use futures::{FutureExt, future};
use parking_lot::RwLock;
use serde_json::{Map, Value};
use tower::ServiceExt;
use tracing::{error, info, warn};

use crate::context::{EventContext, HandlerContext, PluginContext, ServiceMap};
use crate::error::EventSkipped;
use crate::plugin::{AMIRA_PLUGIN_API_VERSION, Plugin, PluginDescriptor, PluginLoadContext};

// =============================================================================
// Topological sort utility
// =============================================================================

/// Computes the plugin load order as **layers** via Kahn's algorithm.
///
/// Returns `Vec<layer>` where each inner `Vec<String>` contains the names of
/// plugins that may be loaded **in parallel** (no intra-layer dependencies).
/// Unload order is obtained by reversing the slice of layers.
///
/// Dependency edges are derived from [`Plugin::provides`] / [`Plugin::depends_on`]:
/// - An edge **A → B** means "A must load before B".
///
/// # Warnings
///
/// - Unresolved dependencies are logged; loading continues without the
///   ordering guarantee for that edge.
/// - Duplicate providers are logged; the last registration wins.
///
/// # Errors
///
/// Returns `None` when a dependency cycle is detected.
fn topological_layers(plugins: &HashMap<String, Arc<Plugin>>) -> Option<Vec<Vec<String>>> {
    let plugin_names: Vec<String> = plugins.keys().cloned().collect();

    // Map: service_id → plugin_name that provides it (last wins).
    let mut provider_map: HashMap<&str, String> = HashMap::new();
    for (name, plugin) in plugins {
        for service_id in plugin.provides() {
            if let Some(prev_name) = provider_map.insert(service_id, name.clone()) {
                warn!(
                    service       = service_id,
                    prev_provider = %prev_name,
                    new_provider  = %name,
                    "Duplicate service provider — last registration wins"
                );
            }
        }
    }

    // Build adjacency / in-degree tables (using plugin name as key).
    let mut in_degree: HashMap<String, usize> =
        plugin_names.iter().map(|n| (n.clone(), 0)).collect();
    let mut dependents: HashMap<String, Vec<String>> = plugin_names
        .iter()
        .map(|n| (n.clone(), Vec::new()))
        .collect();

    for (name, plugin) in plugins {
        for entry in plugin.depends_on() {
            if let Some(provider_name) = provider_map.get(entry.name) {
                if provider_name == name {
                    warn!(
                        plugin  = %name,
                        service = entry.name,
                        "Plugin depends on a service it provides itself — ignored"
                    );
                } else {
                    dependents
                        .get_mut(provider_name)
                        .unwrap()
                        .push(name.clone());
                    *in_degree.get_mut(name).unwrap() += 1;
                }
            }
        }
    }

    // Kahn's algorithm — collect one layer per BFS frontier.
    let mut layers: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = plugin_names
        .iter()
        .filter(|n| in_degree.get(*n).is_some_and(|&d| d == 0))
        .cloned()
        .collect();
    let mut processed = 0;

    while !current.is_empty() {
        processed += current.len();
        let mut next: Vec<String> = Vec::new();
        for name in &current {
            for dependent in &dependents[name] {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(dependent.clone());
                    }
                }
            }
        }
        layers.push(current);
        current = next;
    }

    if processed != plugins.len() {
        let cycle_nodes: Vec<String> = plugin_names
            .iter()
            .filter(|n| in_degree.get(*n).is_some_and(|&d| d > 0))
            .cloned()
            .collect();
        error!(
            cycle_nodes = ?cycle_nodes,
            "Plugin dependency cycle detected"
        );
        return None;
    }

    Some(layers)
}

/// Tracks the load/activation state of a plugin registered with [`PluginManager`].
///
/// The state machine is:
///
/// ```text
/// register_plugin() ──► Registered
///     load_all()  ──► Active    (deps met, on_load succeeded)
///                  ──► Failed    (deps missing; plugin skipped)
///     unload_all()   ──► Registered (Active → Registered after on_unload)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLoadState {
    /// Registered but not yet activated (default after `register_plugin`).
    Registered,
    /// Successfully loaded — participating in event dispatch and service provision.
    Active,
    /// Could not be loaded (e.g. a declared dependency was absent in the global
    /// service registry).  Handlers and services from this plugin are ignored.
    Failed,
}

// =============================================================================
// PluginEntry (internal)
// =============================================================================

struct PluginEntry {
    plugin: Arc<Plugin>,
    state: PluginLoadState,
    context: Arc<PluginContext>,
}

// =============================================================================
// PluginManager
// =============================================================================

/// Central manager for plugin registration, lifecycle, and event dispatch.
///
/// All plugin-related operations that were previously spread across the runtime
/// are encapsulated here.  The runtime holds an `Arc<PluginManager>` and
/// passes it to [`AdapterBridge`](amira_core::AdapterBridge) as an
/// `Arc<dyn Dispatcher>`.
///
/// # Global service map
///
/// [`PluginManager`] directly owns a `HashMap` of all inter-plugin services.
/// When a plugin is loaded its declared service factories are called and the
/// results are inserted into this map.  When the plugin is unloaded those
/// entries are removed.
///
/// # Plugin configuration
///
/// `plugin_configs` is a map from plugin name → `serde_json::Value` extracted
/// from `amira.toml → plugins → <name>`.  The runtime converts the figment
/// config before calling [`new`](Self::new).
pub struct PluginManager {
    plugins: RwLock<HashMap<String, PluginEntry>>,
    /// Per-plugin config sections, keyed by plugin name. Stored as Arc<Value> to avoid cloning.
    plugin_configs: HashMap<String, Arc<Value>>,
    /// Managed exclusively by [`load_all`] / [`unload_all`].
    /// Wrapped in Arc for sharing with PluginContext instances.
    services: Arc<RwLock<ServiceMap>>,
    /// Runtime-scoped command context.
    #[cfg(feature = "command")]
    command_context: Arc<crate::context::CommandContext>,
}

impl PluginManager {
    /// Creates a new manager with the given per-plugin config map.
    pub fn new(
        plugin_configs: HashMap<String, Value>,
        #[cfg(feature = "command")] command_config: crate::command::CommandConfig,
    ) -> Self {
        Self {
            plugins: RwLock::default(),
            plugin_configs: plugin_configs
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            services: Arc::default(),
            #[cfg(feature = "command")]
            command_context: Arc::new(crate::context::CommandContext {
                config: command_config,
                help_provider: parking_lot::Mutex::default(),
            }),
        }
    }

    // ─── Plugin registration ─────────────────────────────────────────────────

    /// Registers a plugin from a [`PluginDescriptor`].
    ///
    /// The plugin is instantiated and stored with state
    /// [`PluginLoadState::Registered`].  It is **not** loaded until
    /// [`load_all`](Self::load_all) is called.
    ///
    /// Logs a warning when the API version does not match, but continues —
    /// hard rejection can be enforced by callers if needed.
    pub fn register_plugin(&self, desc: &PluginDescriptor) -> Arc<PluginContext> {
        if !desc.is_compatible() {
            warn!(
                plugin = %desc.name,
                descriptor_version = %format!(
                    "{}.{}",
                    desc.api_version >> 16,
                    desc.api_version & 0xFFFF
                ),
                host_version = %format!(
                    "{}.{}",
                    AMIRA_PLUGIN_API_VERSION >> 16,
                    AMIRA_PLUGIN_API_VERSION & 0xFFFF
                ),
                "Plugin API version mismatch — registering anyway, but behaviour may be undefined"
            );
        }
        let instance = desc.instantiate();
        let name = instance.name().to_string();

        let config = self
            .plugin_configs
            .get(&name)
            .cloned()
            .unwrap_or_else(|| Arc::new(Value::Object(Map::default())));

        // Build the list of enabled service IDs (both provides and depends_on)
        let service_ids: HashSet<String> = instance
            .depends_on()
            .iter()
            .map(|entry| &entry.name)
            .chain(instance.provides().iter())
            .map(ToString::to_string)
            .collect();

        let context = Arc::new(PluginContext::new(
            name.clone(),
            config,
            service_ids,
            self.services.clone(),
        ));

        self.plugins.write().insert(
            name.clone(),
            PluginEntry {
                plugin: Arc::new(instance),
                state: PluginLoadState::Registered,
                context: context.clone(),
            },
        );
        info!(plugin = %name, "Plugin registered");

        context
    }

    /// Removes the first plugin whose name matches `name`.
    ///
    /// If the runtime is already running, call [`unload_all`](Self::unload_all)
    /// first to invoke the plugin's `on_unload` hook.
    ///
    /// Returns `false` if the plugin is not found or if it is currently active.
    pub fn remove_plugin(&self, name: &str) -> bool {
        let mut plugins = self.plugins.write();
        if let Some(entry) = plugins.get(name)
            && entry.state == PluginLoadState::Active
        {
            error!(
                plugin = %name,
                "Cannot remove plugin — it is currently active. Call unload_all first."
            );
            return false;
        }
        if plugins.remove(name).is_some() {
            info!(plugin = %name, "Plugin removed");
            true
        } else {
            false
        }
    }

    /// Returns the number of registered plugins (in any state).
    pub fn plugin_count(&self) -> usize {
        self.plugins.read().len()
    }

    /// Returns a map of plugin name → load state for all registered plugins.
    pub fn plugin_states(&self) -> HashMap<String, PluginLoadState> {
        self.plugins
            .read()
            .iter()
            .map(|(name, entry)| (name.clone(), entry.state))
            .collect()
    }

    /// Sets a plugin's load state. Returns `true` if successful, `false` if not found.
    fn set_plugin_state(&self, name: &str, state: PluginLoadState) -> bool {
        if let Some(entry) = self.plugins.write().get_mut(name) {
            entry.state = state;
            true
        } else {
            false
        }
    }

    /// Loads a single plugin in dependency order.
    ///
    /// If the plugin is already in `Active` state, returns `true` immediately.
    /// Returns `false` on any failure (missing required dependencies, `on_load` error, etc.);
    /// returns `true` on success.
    ///
    /// The PluginContext was created at registration time and is reused from there.
    pub async fn load_plugin(&self, name: &str) -> bool {
        // ── 1. Check state and required deps ─────────────────────────────
        let (plugin, ctx) = {
            let plugins = self.plugins.read();
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state == PluginLoadState::Active {
                return true;
            }
            (entry.plugin.clone(), entry.context.clone())
        };

        // Check only required dependencies — optional deps missing is not an error
        let missing = {
            let svc_guard = self.services.read();
            let id_set: HashSet<&str> = svc_guard.values().map(|(id, _)| id.as_str()).collect();
            plugin
                .depends_on()
                .iter()
                .find(|entry| entry.required && !id_set.contains(entry.name))
                .map(|entry| entry.name)
        };

        if let Some(dep) = missing {
            error!(
                plugin = %name,
                missing_dependency = %dep,
                "Required plugin dependency not satisfied — plugin will not be loaded"
            );
            self.set_plugin_state(name, PluginLoadState::Failed);
            return false;
        }

        // ── 2. Initialise services in parallel ───────────────────────────
        let all_services = future::join_all(plugin.service_entries().iter().map(async |entry| {
            let id = entry.id.to_string();

            match tokio::spawn((entry.factory)(ctx.clone())).await {
                Ok(Ok(arc)) => Ok((entry.type_id, (id, arc))),
                Ok(Err(e)) => Err((id, e)),
                Err(panic) => Err((id, panic.to_string())),
            }
        }))
        .await;

        // Check if any service initialization failed
        for result in &all_services {
            if let Err((svc_id, e)) = result {
                error!(
                    plugin = %name,
                    service_id = %svc_id,
                    error = %e,
                    "Service initialization failed — plugin will not be loaded"
                );
                self.set_plugin_state(name, PluginLoadState::Failed);
                return false;
            }
        }

        // Register all services
        {
            let mut svc_map = self.services.write();
            for (id, service) in all_services.into_iter().flatten() {
                svc_map.insert(id, service);
            }
        }

        // ── 3. on_load ───────────────────────────────────────────────────
        let load_ctx = PluginLoadContext::new(
            ctx.clone(),
            #[cfg(feature = "command")]
            self.command_context.clone(),
        );

        match tokio::spawn(async move { plugin.on_load(load_ctx).await }).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                error!(
                    plugin = %name,
                    error  = %e,
                    "Plugin on_load returned an error — plugin will not be loaded"
                );
                self.set_plugin_state(name, PluginLoadState::Failed);
                return false;
            }
            Err(panic) => {
                error!(
                    plugin = %name,
                    error  = %panic,
                    "Plugin on_load panicked — plugin will not be loaded"
                );
                self.set_plugin_state(name, PluginLoadState::Failed);
                return false;
            }
        }

        // ── 4. Mark Active ───────────────────────────────────────────────
        if self.set_plugin_state(name, PluginLoadState::Active) {
            info!(plugin = %name, "Plugin loaded and active");
            return true;
        }
        false
    }

    /// Unloads a single plugin without checking for dependent plugins.
    ///
    /// This is an internal method used by [`unload_all`] which respects dependency order.
    /// Returns `true` on success; `false` if the plugin is not found or not active.
    async fn unload_plugin_unchecked(&self, name: &str) -> bool {
        let plugin = {
            let plugins = self.plugins.read();
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state != PluginLoadState::Active {
                return false;
            }
            entry.plugin.clone()
        };

        // Run on_unload hook.
        plugin.on_unload().await;

        // Remove services.
        {
            let mut svc_map = self.services.write();
            for entry in plugin.service_entries() {
                svc_map.remove(&entry.type_id);
            }
        }

        #[cfg(feature = "command")]
        self.command_context.help_provider.lock().remove(name);

        // Mark as Registered.
        if self.set_plugin_state(name, PluginLoadState::Registered) {
            info!(plugin = %name, "Plugin unloaded");
            return true;
        }

        false
    }

    /// Unloads a single plugin if no other active plugins depend on its services.
    ///
    /// Returns `true` on success; `false` if the plugin is not found, not active,
    /// or if other active plugins depend on its services.
    pub async fn unload_plugin(&self, name: &str) -> bool {
        // Check if plugin exists and is active.
        let plugin = {
            let plugins = self.plugins.read();
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state != PluginLoadState::Active {
                return false;
            }
            entry.plugin.clone()
        };

        let plugin_services = plugin.provides();

        // Check if any other active plugin has required dependencies on this plugin's services.
        for (other_name, entry) in self.plugins.read().iter() {
            if other_name == name || entry.state != PluginLoadState::Active {
                continue;
            }
            for dep in entry.plugin.depends_on() {
                if dep.required && plugin_services.contains(&dep.name) {
                    error!(
                        plugin = %name,
                        dependent = %other_name,
                        service = %dep.name,
                        "Cannot unload plugin — other active plugins require its services"
                    );
                    return false;
                }
            }
        }

        // Dependency check passed; call internal unchecked version.
        self.unload_plugin_unchecked(name).await
    }

    /// Attempts to load all registered plugins in dependency order.
    pub async fn load_all(&self) {
        let layers = {
            let plugins = self.plugins.read();
            let plugins_map = plugins
                .iter()
                .map(|(name, entry)| (name.clone(), entry.plugin.clone()))
                .collect::<HashMap<_, _>>();
            if let Some(l) = topological_layers(&plugins_map) {
                l
            } else {
                error!("Skipping plugin loading due to dependency cycle");
                return;
            }
        };

        for layer in layers {
            future::join_all(layer.iter().map(|name| self.load_plugin(name))).await;
        }
    }

    /// Unloads all **active** plugins in reverse dependency order.
    pub async fn unload_all(&self) {
        let layers = {
            let plugins = self.plugins.read();
            let plugins_map = plugins
                .iter()
                .filter(|(_, entry)| entry.state == PluginLoadState::Active)
                .map(|(name, entry)| (name.clone(), entry.plugin.clone()))
                .collect::<HashMap<_, _>>();
            if let Some(l) = topological_layers(&plugins_map) {
                l
            } else {
                error!("Skipping plugin unloading due to dependency cycle");
                return;
            }
        };

        for layer in layers.iter().rev() {
            future::join_all(layer.iter().map(|name| self.unload_plugin_unchecked(name))).await;
        }
    }
}

// =============================================================================
// Dispatcher impl
// =============================================================================

impl Dispatcher for PluginManager {
    /// Dispatches `event` to all **active** plugins' handlers in a single flat
    /// list, executed **sequentially**.  Each handler runs in its own spawned
    /// task for an isolated runtime environment.  If any handler calls
    /// `stop_propagation`, the remaining handlers are skipped.
    async fn dispatch<E, B>(&self, event: E, bot: Arc<B>)
    where
        E: EventRoot,
        B: Bot,
    {
        let base = Arc::new(EventContext::new(
            Arc::new(event),
            bot,
            #[cfg(feature = "command")]
            self.command_context.clone(),
        ));

        // Collect all handlers from all active plugins into a single flat list.
        let all_handlers: Vec<_> = {
            let plugins = self.plugins.read();
            plugins
                .iter()
                .filter(|(_, e)| e.state == PluginLoadState::Active)
                .flat_map(|(_, e)| {
                    e.plugin
                        .handlers()
                        .iter()
                        .cloned()
                        .map(|svc| (svc, e.context.clone()))
                })
                .collect()
        };

        // Execute handlers sequentially; each runs in its own spawned task.
        for (svc, plugin_ctx) in all_handlers {
            if !base.is_propagating() {
                break;
            }

            let handler_ctx = HandlerContext::new(base.clone(), plugin_ctx.clone());

            match AssertUnwindSafe(svc.oneshot(handler_ctx))
                .catch_unwind()
                .await
            {
                Ok(Err(e)) if !e.is::<EventSkipped>() => {
                    error!(plugin = plugin_ctx.name(), error = %e, "Handler returned an error");
                }
                Err(_) => {
                    error!(plugin = plugin_ctx.name(), "Handler panicked");
                }
                _ => {}
            }
        }
    }
}
