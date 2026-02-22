//! Plugin lifecycle management and event dispatch.
//!
//! [`PluginManager`] is the central owner of all registered plugins. It:
//!
//! - Accepts [`PluginDescriptor`]s and instantiates them into live [`Plugin`]s
//!   with an initial state of [`PluginLoadState::Registered`].
//! - Drives plugin lifecycle (`on_load` / `on_unload`) in dependency order via
//!   [`start_all`](PluginManager::start_all) / [`stop_all`](PluginManager::stop_all).
//! - On `start_all`, checks that every declared dependency is satisfied;
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
//! manager.start_all().await;
//! // …later…
//! manager.stop_all().await;
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
use crate::plugin::{ALLOY_PLUGIN_API_VERSION, Plugin, PluginDescriptor};
use alloy_core::{BoxedBot, BoxedEvent, Dispatcher};

// =============================================================================
// Topological sort utility
// =============================================================================

/// Computes the plugin load order as **layers** via Kahn's algorithm.
///
/// Returns `Vec<layer>` where each inner `Vec<usize>` contains the indices of
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
/// Returns `Err(description)` when a dependency cycle is detected.
fn topological_layers(plugins: &[Arc<Plugin>]) -> Result<Vec<Vec<usize>>, String> {
    let n = plugins.len();

    // Map: service_id → index of the plugin that provides it (last wins).
    let mut provider_map: HashMap<&str, usize> = HashMap::new();
    for (i, plugin) in plugins.iter().enumerate() {
        for service_id in plugin.provides() {
            if let Some(prev) = provider_map.insert(service_id, i) {
                warn!(
                    service       = service_id,
                    prev_provider = %plugins[prev].name(),
                    new_provider  = %plugin.name(),
                    "Duplicate service provider — last registration wins"
                );
            }
        }
    }

    // Build adjacency / in-degree tables.
    let mut in_degree: Vec<usize> = vec![0; n];
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; n];

    for (i, plugin) in plugins.iter().enumerate() {
        for &dep_id in plugin.depends_on() {
            match provider_map.get(dep_id) {
                Some(&provider) if provider != i => {
                    dependents[provider].push(i);
                    in_degree[i] += 1;
                }
                Some(_) => {
                    warn!(
                        plugin  = %plugin.name(),
                        service = dep_id,
                        "Plugin depends on a service it provides itself — ignored"
                    );
                }
                None => {
                    warn!(
                        plugin     = %plugin.name(),
                        dependency = dep_id,
                        "Unresolved dependency — no loaded plugin provides '{dep_id}'; \
                         load order for this dependency is not guaranteed"
                    );
                }
            }
        }
    }

    // Kahn's algorithm — collect one layer per BFS frontier.
    let mut layers: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut processed = 0;

    while !current.is_empty() {
        processed += current.len();
        let mut next: Vec<usize> = Vec::new();
        for &i in &current {
            for &j in &dependents[i] {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    next.push(j);
                }
            }
        }
        layers.push(current);
        current = next;
    }

    if processed != n {
        let cycle_nodes: Vec<String> = (0..n)
            .filter(|&i| in_degree[i] > 0)
            .map(|i| plugins[i].name().to_string())
            .collect();
        return Err(format!(
            "Plugin dependency cycle detected among: {}",
            cycle_nodes.join(", ")
        ));
    }

    Ok(layers)
}

/// Tracks the load/activation state of a plugin registered with [`PluginManager`].
///
/// The state machine is:
///
/// ```text
/// register_plugin() ──► Registered
///     start_all()  ──► Active    (deps met, on_load succeeded)
///                  ──► Failed    (deps missing; plugin skipped)
///     stop_all()   ──► Registered (Active → Registered after on_unload)
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
    plugins: AsyncRwLock<Vec<PluginEntry>>,
    /// Per-plugin config sections, keyed by plugin name.
    plugin_configs: HashMap<String, Value>,
    /// Managed exclusively by [`start_all`] / [`stop_all`].
    services: RwLock<HashMap<String, (TypeId, Arc<dyn Any + Send + Sync>)>>,
}

impl PluginManager {
    /// Creates a new manager with the given per-plugin config map.
    pub fn new(plugin_configs: HashMap<String, Value>) -> Self {
        Self {
            plugins: AsyncRwLock::new(Vec::new()),
            plugin_configs,
            services: RwLock::new(HashMap::new()),
        }
    }

    // ─── Plugin registration ─────────────────────────────────────────────────

    /// Registers a plugin from a [`PluginDescriptor`].
    ///
    /// The plugin is instantiated and stored with state
    /// [`PluginLoadState::Registered`].  It is **not** loaded until
    /// [`start_all`](Self::start_all) is called.
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
        self.plugins.write().await.push(PluginEntry {
            plugin: Arc::new(instance),
            state: PluginLoadState::Registered,
        });
        info!(plugin = %name, "Plugin registered");
    }

    /// Removes the first plugin whose name matches `name`.
    ///
    /// If the runtime is already running, call [`stop_all`](Self::stop_all)
    /// first to invoke the plugin's `on_unload` hook.
    pub async fn remove_plugin(&self, name: &str) {
        let mut plugins = self.plugins.write().await;
        if let Some(pos) = plugins.iter().position(|e| e.plugin.name() == name) {
            plugins.remove(pos);
            info!(plugin = %name, "Plugin removed");
        }
    }

    /// Returns the number of registered plugins (in any state).
    pub async fn plugin_count(&self) -> usize {
        self.plugins.read().await.len()
    }

    /// Returns the load state of the named plugin, or `None` if not found.
    pub async fn plugin_state(&self, name: &str) -> Option<PluginLoadState> {
        self.plugins
            .read()
            .await
            .iter()
            .find(|e| e.plugin.name() == name)
            .map(|e| e.state)
    }

    /// Attempts to load all registered plugins in dependency order.
    pub async fn start_all(&self) {
        let layers = {
            let list = self.plugins.read().await;
            let plugins_ref: Vec<Arc<Plugin>> =
                list.iter().map(|e| Arc::clone(&e.plugin)).collect();
            match topological_layers(&plugins_ref) {
                Ok(l) => l,
                Err(e) => {
                    error!("{e}");
                    (0..list.len()).map(|i| vec![i]).collect()
                }
            }
        };

        for layer in layers {
            // ── 1. Classify: skip non-Registered, check dependencies ──────
            let mut failed: Vec<usize> = Vec::new();
            let mut to_load: Vec<(usize, Arc<Plugin>, serde_json::Value)> = Vec::new();
            {
                let list = self.plugins.read().await;
                let svc_guard = self.services.read().unwrap();
                for &i in &layer {
                    let entry = &list[i];
                    if entry.state != PluginLoadState::Registered {
                        continue;
                    }
                    let plugin = Arc::clone(&entry.plugin);
                    let name = plugin.name().to_string();
                    let missing = plugin
                        .depends_on()
                        .iter()
                        .find(|dep| !svc_guard.contains_key(**dep))
                        .copied();
                    if let Some(dep) = missing {
                        error!(
                            plugin = %name,
                            missing_dependency = %dep,
                            "Plugin dependency not satisfied — plugin will not be loaded"
                        );
                        failed.push(i);
                    } else {
                        let config = self
                            .plugin_configs
                            .get(&name)
                            .cloned()
                            .unwrap_or_else(|| Value::Object(Map::default()));
                        to_load.push((i, plugin, config));
                    }
                }
            }

            // Mark failures.
            if !failed.is_empty() {
                let mut list = self.plugins.write().await;
                for i in failed {
                    list[i].state = PluginLoadState::Failed;
                }
            }

            if to_load.is_empty() {
                continue;
            }

            // ── 2. Initialise all services across the layer in parallel ────
            let all_services = future::join_all(to_load.iter().flat_map(|(_, plugin, config)| {
                plugin
                    .service_factories()
                    .iter()
                    .map(|entry| {
                        let factory = Arc::clone(&entry.factory);
                        let id = entry.id.to_string();
                        let type_id = entry.type_id;
                        let config = config.clone();
                        async move {
                            let arc = factory(config).await;
                            (id, (type_id, arc))
                        }
                    })
                    .collect::<Vec<_>>()
            }))
            .await;

            // Batch insert under a single lock.
            {
                let mut svc_map = self.services.write().unwrap();
                for (id, service) in all_services {
                    svc_map.insert(id, service);
                }
            }

            // ── 3. Run on_load hooks across the layer in parallel ─────────
            future::join_all(to_load.iter().map(|(_, plugin, config)| {
                let plugin = Arc::clone(plugin);
                let config = config.clone();
                async move { plugin.on_load(config).await }
            }))
            .await;

            // ── 4. Mark all as Active ─────────────────────────────────────
            {
                let mut list = self.plugins.write().await;
                for (i, plugin, _) in &to_load {
                    list[*i].state = PluginLoadState::Active;
                    info!(plugin = %plugin.name(), "Plugin loaded and active");
                }
            }
        }
    }

    /// Unloads all **active** plugins in reverse dependency order.
    pub async fn stop_all(&self) {
        let layers = {
            let list = self.plugins.read().await;
            let plugins_ref: Vec<Arc<Plugin>> =
                list.iter().map(|e| Arc::clone(&e.plugin)).collect();
            let mut layers = match topological_layers(&plugins_ref) {
                Ok(l) => l,
                Err(_) => (0..list.len()).map(|i| vec![i]).collect(),
            };
            layers.reverse();
            layers
        };

        for layer in layers {
            // Collect active plugins to unload in this layer.
            let to_unload: Vec<(usize, Arc<Plugin>)> = {
                let list = self.plugins.read().await;
                layer
                    .iter()
                    .filter_map(|&i| {
                        let entry = &list[i];
                        (entry.state == PluginLoadState::Active)
                            .then(|| (i, Arc::clone(&entry.plugin)))
                    })
                    .collect()
            };

            if to_unload.is_empty() {
                continue;
            }

            // Run on_unload hooks in parallel.
            future::join_all(to_unload.iter().map(|(_, plugin)| {
                let plugin = Arc::clone(plugin);
                async move { plugin.on_unload().await }
            }))
            .await;

            // Batch remove services under a single lock.
            {
                let mut svc_map = self.services.write().unwrap();
                for (_, plugin) in &to_unload {
                    for id in plugin.provides() {
                        svc_map.remove(id);
                    }
                }
            }

            // Mark all as Registered.
            {
                let mut list = self.plugins.write().await;
                for (i, plugin) in &to_unload {
                    list[*i].state = PluginLoadState::Registered;
                    info!(plugin = %plugin.name(), "Plugin unloaded");
                }
            }
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
            let list = self.plugins.read().await;
            list.iter()
                .filter(|e| e.state == PluginLoadState::Active)
                .map(|e| {
                    let name = e.plugin.name().to_string();
                    let cfg = Arc::new(
                        self.plugin_configs
                            .get(&name)
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
