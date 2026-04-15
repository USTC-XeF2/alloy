//! Main runtime orchestration with capability-based transport system.
//!
//! The runtime initializes adapters with a TransportContext containing
//! available transport capabilities. Adapters then use these capabilities
//! to establish connections dynamically.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use alloy_runtime::AlloyRuntime;
//!
//! // Simplest way - auto-loads config from current directory
//! let runtime = AlloyRuntime::new();
//!
//! // Use pre-loaded config
//! let config = load_config()?;
//! let runtime = AlloyRuntime::from_config(&config);
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::future;
use parking_lot::Mutex;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::config::{AlloyConfig, ConfigLoader};
use crate::error::{RuntimeError, RuntimeResult};
use crate::handle::BotHandle;
use crate::logging;
use alloy_core::adapter::Adapter;
use alloy_core::bridge::{AdapterBridge, BridgeRuntime};
use alloy_core::transport::TransportContext;
use alloy_framework::{context::PluginContext, manager::PluginManager, plugin::PluginDescriptor};

/// The main Alloy runtime that orchestrates adapters, transports, and plugins.
///
/// # Simple Usage
///
/// ```rust,ignore
/// use alloy_runtime::AlloyRuntime;
/// use alloy::prelude::*;
///
/// // Auto-loads config from alloy.toml in current directory
/// let runtime = AlloyRuntime::new();
///
/// // Register an adapter (configured from alloy.toml)
/// runtime.register_adapter::<OneBotAdapter>()?;
///
/// // Register a plugin that contains all your handlers
/// runtime.register_plugin(define_plugin! {
///     name: "echo",
///     handlers: [on_message().handler(echo_handler)],
/// }).await;
///
/// runtime.run().await;
/// ```
///
/// # Custom Configuration
///
/// ```rust,ignore
/// let runtime = AlloyRuntime::builder()
///     .config_file("config/production.yaml")
///     .profile("production")
///     .build()?;
/// ```
pub struct AlloyRuntime {
    /// The configuration.
    config: AlloyConfig,
    /// Plugin manager — owns all plugins and drives event dispatch.
    plugin_manager: Arc<PluginManager>,
    /// Transport context.
    transport_context: TransportContext,
    /// Adapter bridges, created eagerly on registration.
    bridges: Arc<Mutex<HashMap<&'static str, Arc<dyn BridgeRuntime>>>>,
    /// Whether the runtime is running.
    running: AtomicBool,
}

impl AlloyRuntime {
    /// Creates a new runtime with automatic configuration loading.
    ///
    /// This will:
    /// 1. Search for `alloy.toml` in the current directory
    /// 2. Initialize logging based on the configuration
    /// 3. Create transport context with all available capabilities
    ///
    /// If no configuration file is found, default settings are used.
    pub fn new() -> Self {
        let config = ConfigLoader::new()
            .with_current_dir()
            .load()
            .unwrap_or_else(|e| {
                warn!("Failed to load configuration: {e}, using defaults");
                AlloyConfig::default()
            });

        Self::from_config(config)
    }

    /// Creates a new runtime from configuration.
    ///
    /// This initializes logging based on the configuration and creates
    /// a TransportContext with all available transport capabilities.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use alloy_runtime::{AlloyRuntime, config::ConfigLoader};
    ///
    /// let config = ConfigLoader::new()
    ///     .with_current_dir()
    ///     .load()?;
    /// let runtime = AlloyRuntime::from_config(config);
    /// ```
    pub fn from_config(config: AlloyConfig) -> Self {
        // Initialize logging from config (try_init won't panic if already initialized)
        let _ = logging::try_init_from_config(&config.logging);

        // Create transport context by collecting all capabilities registered via
        // `#[register_capability(...)]` across linked crates.
        let transport_ctx = TransportContext::collect_all();

        info!("Runtime initialized from configuration");

        let plugin_manager = PluginManager::new(
            config.plugins.clone(),
            #[cfg(feature = "command")]
            config.command.clone(),
        );

        Self {
            config,
            plugin_manager: Arc::new(plugin_manager),
            transport_context: transport_ctx,
            bridges: Arc::default(),
            running: AtomicBool::new(false),
        }
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &AlloyConfig {
        &self.config
    }

    /// Registers an adapter with the runtime.
    ///
    /// Configuration is loaded from `alloy.toml` under the adapter's name key,
    /// or falls back to `Default::default()` if not found.
    /// An [`AdapterBridge`] is created immediately so there is no separate `init` step.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// runtime.register_adapter::<OneBotAdapter>()?;
    /// ```
    pub fn register_adapter<A>(&self) -> RuntimeResult<()>
    where
        A: Adapter,
    {
        let adapter_name = A::NAME;

        // Try to get config from file, otherwise use default
        let config = if let Some(config_value) = self.config.adapters.get(adapter_name) {
            A::Config::deserialize(config_value).map_err(|e| {
                RuntimeError(format!(
                    "Failed to deserialize config for adapter '{adapter_name}': {e}"
                ))
            })?
        } else {
            warn!(
                adapter = adapter_name,
                "No configuration found for adapter, using default"
            );
            Default::default()
        };

        let adapter = A::from_config(config);
        let bridge = Arc::new(AdapterBridge::new(
            adapter,
            self.plugin_manager.clone(),
            self.transport_context,
        ));

        self.bridges.lock().insert(adapter_name, bridge);
        info!(adapter = adapter_name, "Registered adapter");
        Ok(())
    }

    /// Registers a plugin from a [`PluginDescriptor`].
    pub fn register_plugin(&self, desc: &PluginDescriptor) -> Arc<PluginContext> {
        self.plugin_manager.register_plugin(desc)
    }

    /// Returns the number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugin_manager.plugin_count()
    }

    /// Returns a handle that can query bots across all registered adapters.
    pub fn bot_handle(&self) -> BotHandle {
        BotHandle(self.bridges.clone())
    }

    /// Returns whether the runtime is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Starts the runtime.
    pub async fn start(&self) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            warn!("Runtime is already running");
            return;
        }

        info!("Starting Alloy runtime");

        // 1. Start adapters in parallel.
        let futures = self
            .bridges
            .lock()
            .iter()
            .map(|(name, bridge)| {
                let name = *name;
                let bridge = bridge.clone();
                async move {
                    if let Err(e) = bridge.start().await {
                        error!(adapter = %name, error = %e, "Failed to start adapter");
                    } else {
                        info!(adapter = %name, "Adapter started");
                    }
                }
            })
            .collect::<Vec<_>>();
        future::join_all(futures).await;

        // 2. Load plugins.
        self.plugin_manager.load_all().await;

        info!("Runtime started");
    }

    /// Stops the runtime, all plugins, and all adapters.
    ///
    /// Shutdown order:
    /// 1. Call [`Plugin::on_unload`] on every registered plugin.
    /// 2. Shut down all registered adapters in parallel.
    pub async fn stop(&self) {
        if self
            .running
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            warn!("Runtime is not running");
            return;
        }

        info!("Stopping Alloy runtime");

        // 1. Unload plugins in reverse dependency order.
        self.plugin_manager.unload_all().await;

        // 2. Shut down adapters in parallel.
        let futures = self
            .bridges
            .lock()
            .values()
            .map(|bridge| {
                let bridge = bridge.clone();
                async move { bridge.shutdown().await }
            })
            .collect::<Vec<_>>();
        future::join_all(futures).await;

        info!("Runtime stopped");
    }

    /// Runs the runtime and blocks until all bridge-owned listener tasks exit.
    pub async fn run(&self) {
        self.start().await;
        self.wait_all_bridges().await;
        self.stop().await;
    }

    /// Runs the runtime with a custom shutdown future.
    pub async fn run_until<F: Future>(&self, shutdown: F) {
        self.start().await;

        tokio::select! {
            _ = shutdown => {}
            () = self.wait_all_bridges() => {}
        }

        self.stop().await;
    }

    async fn wait_all_bridges(&self) {
        let bridges = self.bridges.lock().values().cloned().collect::<Vec<_>>();
        let futures = bridges
            .iter()
            .map(|bridge| {
                let bridge = bridge.clone();
                async move { bridge.wait().await }
            })
            .collect::<Vec<_>>();
        future::join_all(futures).await;
    }
}

impl Default for AlloyRuntime {
    fn default() -> Self {
        Self::new()
    }
}
