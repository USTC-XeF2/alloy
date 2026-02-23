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
//! // Custom configuration path
//! let runtime = AlloyRuntime::builder()
//!     .config_file("config/alloy.yaml")
//!     .build()?;
//!
//! // Use pre-loaded config
//! let config = load_config()?;
//! let runtime = AlloyRuntime::from_config(&config);
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::future;
use tokio::signal;
use tracing::{error, info, warn};

use crate::config::{AlloyConfig, ConfigLoader};
use crate::error::{ConfigResult, RuntimeError, RuntimeResult};
use crate::logging;
use alloy_core::{AdapterBridge, ConfigurableAdapter, TransportContext};
use alloy_framework::{PluginDescriptor, PluginManager};

/// The main Alloy runtime that orchestrates adapters, transports, and plugins.
///
/// # Simple Usage
///
/// ```rust,ignore
/// use alloy_runtime::AlloyRuntime;
/// use alloy::prelude::*;
///
/// // Auto-loads config from alloy.yaml in current directory
/// let runtime = AlloyRuntime::new();
///
/// // Register an adapter (configured from alloy.yaml)
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
    bridges: Mutex<HashMap<String, Arc<AdapterBridge>>>,
    /// Whether the runtime is running.
    running: AtomicBool,
}

impl AlloyRuntime {
    /// Creates a new runtime with automatic configuration loading.
    ///
    /// This will:
    /// 1. Search for `alloy.yaml` in the current directory
    /// 2. Initialize logging based on the configuration
    /// 3. Create transport context with all available capabilities
    ///
    /// If no configuration file is found, default settings are used.
    pub fn new() -> Self {
        let config = ConfigLoader::new()
            .with_current_dir()
            .load()
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to load config ({e}), using defaults");
                AlloyConfig::default()
            });

        Self::from_config(&config)
    }

    /// Creates a runtime builder for custom configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let runtime = AlloyRuntime::builder()
    ///     .config_file("config/production.yaml")
    ///     .profile("production")
    ///     .build()?;
    /// ```
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Creates a new runtime from configuration.
    ///
    /// This initializes logging based on the configuration and creates
    /// a TransportContext with all available transport capabilities.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use alloy_runtime::{AlloyRuntime, config::load_config};
    ///
    /// let config = load_config()?;
    /// let runtime = AlloyRuntime::from_config(&config);
    /// ```
    pub fn from_config(config: &AlloyConfig) -> Self {
        // Initialize logging from config (try_init won't panic if already initialized)
        logging::init_from_config(&config.logging);

        // Create transport context by collecting all capabilities registered via
        // `#[register_capability(...)]` across linked crates.
        let transport_ctx = alloy_core::TransportContext::collect_all();

        info!(
            log_level = %config.logging.level,
            log_format = ?config.logging.format,
            "Runtime initialized from configuration"
        );

        // Convert plugin configs from figment::value::Value to serde_json::Value
        // so that PluginManager (in alloy-framework) stays free of figment.
        let plugin_configs = config
            .plugins
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or_default()))
            .collect();

        Self {
            config: config.clone(),
            plugin_manager: Arc::new(PluginManager::new(plugin_configs)),
            transport_context: transport_ctx,
            bridges: Mutex::new(HashMap::new()),
            running: AtomicBool::new(false),
        }
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &AlloyConfig {
        &self.config
    }

    /// Registers an adapter with the runtime.
    ///
    /// Configuration is loaded from `alloy.yaml` under the adapter's name key,
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
        A: ConfigurableAdapter + 'static,
    {
        let adapter_name = A::name();

        // Try to get config from file, otherwise use default
        let config: A::Config = if let Some(config_value) = self.config.adapters.get(adapter_name) {
            config_value.clone().deserialize().map_err(|e| {
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

        let adapter = Arc::new(A::from_config(config));
        let bridge = Arc::new(AdapterBridge::new(
            adapter,
            self.plugin_manager.clone(),
            self.transport_context.clone(),
        ));

        self.bridges
            .lock()
            .unwrap()
            .insert(adapter_name.to_string(), bridge);
        info!(adapter = adapter_name, "Registered adapter");
        Ok(())
    }

    /// Registers a plugin from a [`PluginDescriptor`].
    ///
    /// Delegates to the underlying [`PluginManager`].
    ///
    /// Because [`PluginDescriptor`] is `Copy`, it can be stored in a `static`
    /// and imported from any module or crate:
    ///
    /// ```rust,ignore
    /// use my_plugin::MY_PLUGIN;
    /// runtime.register_plugin(MY_PLUGIN).await;
    /// ```
    pub async fn register_plugin(&self, desc: PluginDescriptor) {
        self.plugin_manager.register_plugin(desc).await;
    }

    /// Returns the number of registered plugins.
    pub async fn plugin_count(&self) -> usize {
        self.plugin_manager.plugin_count().await
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
            .unwrap()
            .iter()
            .map(|(name, bridge)| {
                let name = name.clone();
                let bridge = bridge.clone();
                async move {
                    if let Err(e) = bridge.on_start().await {
                        error!(adapter = %name, error = %e, "Failed to start adapter");
                    } else {
                        info!(adapter = %name, "Adapter started");
                    }
                }
            })
            .collect::<Vec<_>>();
        future::join_all(futures).await;

        // 2. Load plugins in dependency order (topological sort).
        self.plugin_manager.start_all().await;

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

        // 1. Unload plugins in reverse dependency order (dependents before providers).
        self.plugin_manager.stop_all().await;

        // 2. Shut down adapters in parallel.
        let futures = self
            .bridges
            .lock()
            .unwrap()
            .iter()
            .map(|(name, bridge)| {
                let name = name.clone();
                let bridge = bridge.clone();
                async move {
                    if let Err(e) = bridge.on_shutdown().await {
                        error!(adapter = %name, error = %e, "Error during adapter shutdown");
                    }
                }
            })
            .collect::<Vec<_>>();
        future::join_all(futures).await;

        info!("Runtime stopped");
    }

    /// Runs the runtime until a shutdown signal is received.
    pub async fn run(&self) {
        self.start().await;

        info!("Alloy runtime is now running. Press Ctrl+C to stop.");

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        self.stop().await;
    }

    /// Runs the runtime with a custom shutdown future.
    pub async fn run_until<F>(&self, shutdown: F)
    where
        F: std::future::Future<Output = ()>,
    {
        self.start().await;

        shutdown.await;

        self.stop().await;
    }

    /// Waits for shutdown signals (Ctrl+C or SIGTERM).
    async fn wait_for_shutdown(&self) {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to register SIGTERM handler");

            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down");
                }
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, shutting down");
                }
            }
        }

        #[cfg(not(unix))]
        {
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            info!("Received Ctrl+C, shutting down");
        }
    }
}

impl Default for AlloyRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// RuntimeBuilder
// =============================================================================

/// Builder for creating an `AlloyRuntime` with custom configuration.
///
/// # Example
///
/// ```rust,ignore
/// let runtime = AlloyRuntime::builder()
///     .config_file("config/production.yaml")
///     .profile("production")
///     .build()?;
/// ```
pub struct RuntimeBuilder {
    config_loader: ConfigLoader,
}

impl RuntimeBuilder {
    /// Creates a new runtime builder.
    pub fn new() -> Self {
        Self {
            config_loader: ConfigLoader::new().with_current_dir(),
        }
    }

    /// Sets a specific configuration file to load.
    pub fn config_file<P: AsRef<std::path::Path>>(mut self, path: P) -> Self {
        self.config_loader = self.config_loader.file(path);
        self
    }

    /// Sets the configuration profile (e.g., "development", "production").
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.config_loader = self.config_loader.profile(profile);
        self
    }

    /// Adds a search path for configuration files.
    pub fn search_path<P: AsRef<std::path::Path>>(mut self, path: P) -> Self {
        self.config_loader = self.config_loader.search_path(path);
        self
    }

    /// Enables loading environment variables (enabled by default).
    pub fn with_env(mut self) -> Self {
        self.config_loader = self.config_loader.with_env();
        self
    }

    /// Disables loading environment variables.
    pub fn without_env(mut self) -> Self {
        self.config_loader = self.config_loader.without_env();
        self
    }

    /// Merges additional configuration programmatically.
    pub fn merge(mut self, config: AlloyConfig) -> Self {
        self.config_loader = self.config_loader.merge(config);
        self
    }

    /// Builds the runtime.
    pub fn build(self) -> ConfigResult<AlloyRuntime> {
        let config = self.config_loader.load()?;
        Ok(AlloyRuntime::from_config(&config))
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
