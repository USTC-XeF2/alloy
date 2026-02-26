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
//!   A read-only snapshot is passed to each [`AlloyContext`] during dispatch.
//! - Implements [`Dispatcher`]: on each incoming event it invokes all **active**
//!   plugins **sequentially** in registration order, sharing a single
//!   [`BaseContext`](crate::context::BaseContext).  Any plugin may call
//!   `stop_propagation` to short-circuit the remaining plugins.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_framework::manager::PluginManager;
//!
//! let manager = Arc::new(PluginManager::new(HashMap::new()));
//! manager.register_plugin(MY_PLUGIN).await;
//! manager.load_all().await;
//! // …later…
//! manager.unload_all().await;
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use futures::future;
use serde_json::{Map, Value};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{error, info, span, warn};

use crate::context::{AlloyContext, BaseContext, PluginContext, ServiceSnapshot};
use crate::plugin::{ALLOY_PLUGIN_API_VERSION, Plugin, PluginDescriptor, PluginLoadContext};
use alloy_core::{BoxedBot, BoxedEvent, Dispatcher};

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
        for &dep_id in plugin.depends_on() {
            match provider_map.get(dep_id) {
                Some(provider_name) if provider_name != name => {
                    dependents
                        .get_mut(provider_name)
                        .unwrap()
                        .push(name.clone());
                    *in_degree.get_mut(name).unwrap() += 1;
                }
                Some(_) => {
                    warn!(
                        plugin  = %name,
                        service = dep_id,
                        "Plugin depends on a service it provides itself — ignored"
                    );
                }
                None => {
                    warn!(
                        plugin     = %name,
                        dependency = dep_id,
                        "Unresolved dependency — no loaded plugin provides '{dep_id}'; \
                         load order for this dependency is not guaranteed"
                    );
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
}

// =============================================================================
// PluginManager
// =============================================================================

/// Central manager for plugin registration, lifecycle, and event dispatch.
///
/// All plugin-related operations that were previously spread across the runtime
/// are encapsulated here.  The runtime holds an `Arc<PluginManager>` and
/// passes it to [`AdapterBridge`](alloy_core::AdapterBridge) as an
/// `Arc<dyn Dispatcher>`.
///
/// # Global service map
///
/// [`PluginManager`] directly owns a `HashMap` of all inter-plugin services.
/// When a plugin is loaded its declared service factories are called and the
/// results are inserted into this map.  When the plugin is unloaded those
/// entries are removed.  During event dispatch a read-only [`ServiceSnapshot`]
/// is created and shared with every [`AlloyContext`].
///
/// # Plugin configuration
///
/// `plugin_configs` is a map from plugin name → `serde_json::Value` extracted
/// from `alloy.yaml → plugins → <name>`.  The runtime converts the figment
/// config before calling [`new`](Self::new).
pub struct PluginManager {
    plugins: AsyncRwLock<HashMap<String, PluginEntry>>,
    /// Per-plugin config sections, keyed by plugin name.
    plugin_configs: HashMap<String, Value>,
    /// Managed exclusively by [`load_all`] / [`unload_all`].
    services: RwLock<HashMap<String, (TypeId, Arc<dyn Any + Send + Sync>)>>,
}

impl PluginManager {
    /// Creates a new manager with the given per-plugin config map.
    pub fn new(plugin_configs: HashMap<String, Value>) -> Self {
        Self {
            plugins: AsyncRwLock::new(HashMap::new()),
            plugin_configs,
            services: RwLock::new(HashMap::new()),
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
    pub async fn register_plugin(&self, desc: PluginDescriptor) {
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
                    ALLOY_PLUGIN_API_VERSION >> 16,
                    ALLOY_PLUGIN_API_VERSION & 0xFFFF
                ),
                "Plugin API version mismatch — registering anyway, but behaviour may be undefined"
            );
        }
        let instance = desc.instantiate();
        let name = instance.name().to_string();
        self.plugins.write().await.insert(
            name.clone(),
            PluginEntry {
                plugin: Arc::new(instance),
                state: PluginLoadState::Registered,
            },
        );
        info!(plugin = %name, "Plugin registered");
    }

    /// Removes the first plugin whose name matches `name`.
    ///
    /// If the runtime is already running, call [`unload_all`](Self::unload_all)
    /// first to invoke the plugin's `on_unload` hook.
    pub async fn remove_plugin(&self, name: &str) {
        if self.plugins.write().await.remove(name).is_some() {
            info!(plugin = %name, "Plugin removed");
        }
    }

    /// Returns the number of registered plugins (in any state).
    pub async fn plugin_count(&self) -> usize {
        self.plugins.read().await.len()
    }

    /// Returns a map of plugin name → load state for all registered plugins.
    pub async fn plugin_states(&self) -> HashMap<String, PluginLoadState> {
        self.plugins
            .read()
            .await
            .iter()
            .map(|(name, entry)| (name.clone(), entry.state))
            .collect()
    }

    /// Loads a single plugin in dependency order.
    ///
    /// If the plugin is already in `Active` state, returns `true` immediately.
    /// Returns `false` on any failure (missing dependencies, `on_load` error, etc.);
    /// returns `true` on success.
    pub async fn load_plugin(&self, name: &str) -> bool {
        // ── 1. Check state and deps ──────────────────────────────────────
        let (plugin, config) = {
            let plugins = self.plugins.read().await;
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state == PluginLoadState::Active {
                return true;
            }
            let plugin = Arc::clone(&entry.plugin);
            let config = self
                .plugin_configs
                .get(name)
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::default()));

            let missing: Option<String> = {
                let svc_guard = self.services.read().unwrap();
                plugin
                    .depends_on()
                    .iter()
                    .find(|dep| !svc_guard.contains_key(**dep))
                    .map(|&s| s.to_string())
            };

            if let Some(dep) = missing {
                error!(
                    plugin = %name,
                    missing_dependency = %dep,
                    "Plugin dependency not satisfied — plugin will not be loaded"
                );
                drop(plugins); // release read lock before acquiring write lock
                if let Some(e) = self.plugins.write().await.get_mut(name) {
                    e.state = PluginLoadState::Failed;
                }
                return false;
            }

            (plugin, config)
        };

        // ── 2. on_load ───────────────────────────────────────────────────
        let ctx = Arc::new(PluginLoadContext::new(config));
        if let Err(e) = plugin.on_load(ctx.clone()).await {
            error!(
                plugin = %name,
                error  = %e,
                "Plugin on_load returned an error — plugin will not be loaded"
            );
            if let Some(entry) = self.plugins.write().await.get_mut(name) {
                entry.state = PluginLoadState::Failed;
            }
            return false;
        }

        // ── 3. Initialise services in parallel ───────────────────────────
        let all_services = future::join_all(plugin.service_factories().iter().map(|entry| {
            let factory = entry.factory.clone();
            let id = entry.id.to_string();
            let type_id = entry.type_id;
            let ctx = ctx.clone();
            async move {
                let arc = factory(ctx).await;
                (id, (type_id, arc))
            }
        }))
        .await;

        {
            let mut svc_map = self.services.write().unwrap();
            for (id, service) in all_services {
                svc_map.insert(id, service);
            }
        }

        // ── 4. Mark Active ───────────────────────────────────────────────
        if let Some(entry) = self.plugins.write().await.get_mut(name) {
            entry.state = PluginLoadState::Active;
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
            let plugins = self.plugins.read().await;
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state != PluginLoadState::Active {
                return false;
            }
            Arc::clone(&entry.plugin)
        };

        // Run on_unload hook.
        plugin.on_unload().await;

        // Remove services.
        {
            let mut svc_map = self.services.write().unwrap();
            for id in plugin.provides() {
                svc_map.remove(id);
            }
        }

        // Mark as Registered.
        if let Some(entry) = self.plugins.write().await.get_mut(name) {
            entry.state = PluginLoadState::Registered;
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
            let plugins = self.plugins.read().await;
            let Some(entry) = plugins.get(name) else {
                return false;
            };
            if entry.state != PluginLoadState::Active {
                return false;
            }
            Arc::clone(&entry.plugin)
        };

        let plugin_services = plugin.provides();

        // Check if any other active plugin depends on this plugin's services.
        for (other_name, entry) in self.plugins.read().await.iter() {
            if other_name == name || entry.state != PluginLoadState::Active {
                continue;
            }
            let other = &entry.plugin;
            for &dep in other.depends_on() {
                if plugin_services.contains(&dep) {
                    error!(
                        plugin = %name,
                        dependent = %other_name,
                        service = %dep,
                        "Cannot unload plugin — other active plugins depend on its services"
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
            let plugins = self.plugins.read().await;
            let plugins_map = plugins
                .iter()
                .map(|(name, entry)| (name.clone(), Arc::clone(&entry.plugin)))
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
            let plugins = self.plugins.read().await;
            let plugins_map = plugins
                .iter()
                .filter(|(_, entry)| entry.state == PluginLoadState::Active)
                .map(|(name, entry)| (name.clone(), Arc::clone(&entry.plugin)))
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

#[async_trait]
impl Dispatcher for PluginManager {
    /// Dispatches `event` to all **active** plugins **sequentially**.
    ///
    /// A single [`BaseContext`] is created and shared (via `Arc`) across every
    /// plugin.  Plugins are invoked in registration order; if any plugin calls
    /// [`AlloyContext::stop_propagation`] the loop exits immediately and
    /// subsequent plugins are skipped.
    async fn dispatch(&self, event: BoxedEvent, bot: BoxedBot) {
        let event_name = event.event_name();

        // Build the service snapshot and shared base context.
        let services = Arc::new(
            self.services
                .read()
                .unwrap()
                .values()
                .map(|(type_id, arc)| (*type_id, Arc::clone(arc)))
                .collect::<ServiceSnapshot>(),
        );
        let base = Arc::new(BaseContext::new(event, bot, services));

        // Snapshot active plugins — brief read lock.
        let active_plugins: Vec<(Arc<Plugin>, Arc<Value>)> = {
            let plugins = self.plugins.read().await;
            plugins
                .iter()
                .filter(|(_, e)| e.state == PluginLoadState::Active)
                .map(|(name, e)| {
                    let cfg = Arc::new(
                        self.plugin_configs
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| Value::Object(Map::default())),
                    );
                    (Arc::clone(&e.plugin), cfg)
                })
                .collect()
        };

        // Dispatch sequentially; stop early if propagation is halted.
        for (plugin, config) in active_plugins {
            if !base.is_propagating() {
                break;
            }

            let plugin_name = plugin.name().to_string();
            let span = span!(
                tracing::Level::DEBUG,
                "dispatch",
                event_name = %event_name,
                plugin = %plugin_name
            );
            let _enter = span.enter();

            let ctx = Arc::new(AlloyContext::new(base.clone(), PluginContext { config }));

            plugin.dispatch_event(ctx).await;
        }
    }
}
