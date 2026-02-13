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
use std::sync::Arc;

use tokio::signal;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::{AlloyConfig, ConfigLoader, ConfigResult};
use crate::error::{RuntimeError, RuntimeResult};
use crate::logging;
use alloy_core::{
    AdapterContext, BotManager, BoxedAdapter, BoxedEvent, ConfigurableAdapter, TransportContext,
};
use alloy_framework::{Dispatcher, Matcher};

/// The main Alloy runtime that orchestrates adapters and bots.
///
/// # Simple Usage
///
/// ```rust,ignore
/// use alloy_runtime::AlloyRuntime;
///
/// // Auto-loads config from alloy.yaml in current directory
/// let runtime = AlloyRuntime::new();
///
/// // Adapter is configured from alloy.yaml, no need to register manually
/// runtime.register_matcher(my_matcher).await;
/// runtime.run().await?;
/// ```
///
/// # Custom Configuration
///
/// ```rust,ignore
/// // Load from specific file
/// let runtime = AlloyRuntime::builder()
///     .config_file("config/production.yaml")
///     .profile("production")
///     .build()?;
///
/// // Or use pre-loaded config
/// let config = load_config_from_file("alloy.yaml")?;
/// let runtime = AlloyRuntime::from_config(&config);
/// ```
pub struct AlloyRuntime {
    /// The configuration.
    config: AlloyConfig,
    /// Map of adapter name to adapter instance.
    adapters: Arc<RwLock<HashMap<String, BoxedAdapter>>>,
    /// The event dispatcher.
    dispatcher: Arc<RwLock<Dispatcher>>,
    /// Transport context (populated before init).
    transport_context: Arc<RwLock<Option<TransportContext>>>,
    /// Adapter contexts (populated after init).
    adapter_contexts: Arc<RwLock<Vec<AdapterContext>>>,
    /// Whether the runtime is running.
    running: Arc<RwLock<bool>>,
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

        // Create transport context with all available capabilities
        let transport_ctx = Self::create_default_transport_context();

        info!(
            log_level = %config.logging.level,
            log_format = ?config.logging.format,
            "Runtime initialized from configuration"
        );

        Self {
            config: config.clone(),
            adapters: Arc::new(RwLock::new(HashMap::new())),
            dispatcher: Arc::new(RwLock::new(Dispatcher::new())),
            transport_context: Arc::new(RwLock::new(Some(transport_ctx))),
            adapter_contexts: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &AlloyConfig {
        &self.config
    }

    /// Creates a default TransportContext with all available transport capabilities.
    ///
    /// This method automatically registers all transport implementations based on
    /// enabled cargo features.
    #[allow(unused_mut)]
    fn create_default_transport_context() -> TransportContext {
        let mut ctx = TransportContext::new();

        // Register WebSocket server capability
        #[cfg(feature = "ws-server")]
        {
            use alloy_transport::websocket::WsServerCapabilityImpl;
            ctx = ctx.with_ws_server(Arc::new(WsServerCapabilityImpl::new()));
            debug!("Registered WsServer capability");
        }

        // Register WebSocket client capability
        #[cfg(feature = "ws-client")]
        {
            use alloy_transport::websocket::WsClientCapabilityImpl;
            ctx = ctx.with_ws_client(Arc::new(WsClientCapabilityImpl::new()));
            debug!("Registered WsClient capability");
        }

        // Register HTTP server capability
        #[cfg(feature = "http-server")]
        {
            use alloy_transport::http::HttpServerCapabilityImpl;
            ctx = ctx.with_http_server(Arc::new(HttpServerCapabilityImpl::new()));
            debug!("Registered HttpServer capability");
        }

        // Register HTTP client capability
        #[cfg(feature = "http-client")]
        {
            use alloy_transport::http::HttpClientCapabilityImpl;
            ctx = ctx.with_http_client(Arc::new(HttpClientCapabilityImpl::new()));
            debug!("Registered HttpClient capability");
        }

        ctx
    }

    /// Sets the transport context.
    ///
    /// This must be called before `init()` to provide transport capabilities
    /// to adapters.
    pub async fn set_transport_context(&self, ctx: TransportContext) {
        let mut guard = self.transport_context.write().await;
        *guard = Some(ctx);
    }

    /// Registers an adapter with the runtime.
    ///
    /// The adapter can be configured via configuration in `alloy.yaml`, or
    /// it will use its default configuration if not found.
    /// The runtime handles all configuration loading and deserialization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// runtime.register_adapter::<OneBotAdapter>().await?;
    /// ```
    ///
    /// This will:
    /// 1. Look for configuration under `adapters.onebot` (from `OneBotAdapter::name()`)
    /// 2. If found, deserialize it into `OneBotAdapter::Config` type
    /// 3. If not found, use the default configuration via `Default::default()`
    /// 4. Call `OneBotAdapter::from_config(config)` to create the adapter
    /// 5. Register the adapter with the runtime
    pub async fn register_adapter<A>(&self) -> RuntimeResult<()>
    where
        A: ConfigurableAdapter + 'static,
    {
        let adapter_name = A::name();

        // Try to get config from file, otherwise use default
        let config: A::Config = match self.config.adapters.get(adapter_name) {
            Some(config_value) => {
                // Deserialize from config
                config_value.clone().deserialize().map_err(|e| {
                    RuntimeError::AdapterConfigDeserialize(format!(
                        "Failed to deserialize config for adapter '{adapter_name}': {e}"
                    ))
                })?
            }
            None => {
                // Use default configuration
                warn!(
                    adapter = adapter_name,
                    "No configuration found for adapter, using default"
                );
                Default::default()
            }
        };

        // Create adapter from its config
        let adapter = A::from_config(config)?;

        let adapter_name = A::name();
        let mut adapters = self.adapters.write().await;
        adapters.insert(adapter_name.to_string(), adapter);
        info!(adapter = adapter_name, "Registered adapter");
        Ok(())
    }

    /// Registers a matcher with the dispatcher.
    ///
    /// Matchers are checked in the order they are added. Each matcher
    /// contains multiple handlers and a check rule.
    pub async fn register_matcher(&self, matcher: Matcher) {
        let mut dispatcher = self.dispatcher.write().await;
        dispatcher.add(matcher);
    }

    /// Registers multiple matchers at once.
    ///
    /// This is a convenience method for adding multiple matchers in one call.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use alloy::prelude::*;
    ///
    /// runtime.register_matchers(vec![
    ///     on_message().handler(log_handler),
    ///     on_command::<EchoCommand>("echo").handler(echo_handler),
    ///     on_command::<HelpCommand>("help").handler(help_handler),
    /// ]).await;
    /// ```
    pub async fn register_matchers(&self, matchers: Vec<Matcher>) {
        let mut dispatcher = self.dispatcher.write().await;
        for matcher in matchers {
            dispatcher.add(matcher);
        }
    }

    /// Returns a reference to the dispatcher.
    pub fn dispatcher(&self) -> &Arc<RwLock<Dispatcher>> {
        &self.dispatcher
    }

    /// Returns whether the runtime is currently running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    // =========================================================================
    // Adapter Management
    // =========================================================================

    /// Gets an adapter by name.
    async fn get_adapter(&self, name: &str) -> Option<BoxedAdapter> {
        let adapters = self.adapters.read().await;
        adapters.get(name).cloned()
    }

    /// Returns all registered adapter names.
    async fn adapter_names(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Creates a BotManager for event dispatching.
    pub fn create_bot_manager(&self) -> Arc<BotManager> {
        let dispatcher = Arc::clone(&self.dispatcher);

        Arc::new(BotManager::new(Arc::new(
            move |event: BoxedEvent, bot: alloy_core::BoxedBot| {
                let dispatcher = Arc::clone(&dispatcher);
                tokio::spawn(async move {
                    let d = dispatcher.read().await;
                    let _ = d.dispatch(event, bot).await;
                });
            },
        )))
    }

    /// Initializes all registered adapters with transport capabilities.
    pub async fn init(&self) -> RuntimeResult<()> {
        // Get transport context (use empty if not set)
        let transport_ctx = {
            let guard = self.transport_context.read().await;
            guard.clone().unwrap_or_default()
        };

        let adapter_names = self.adapter_names().await;
        debug!("Initializing {} adapter(s)", adapter_names.len());

        let mut contexts = self.adapter_contexts.write().await;

        for name in adapter_names {
            if self.get_adapter(&name).await.is_some() {
                // Create bot manager for this adapter
                let bot_manager = self.create_bot_manager();

                // Create adapter context
                let ctx = AdapterContext::new(transport_ctx.clone(), bot_manager);

                debug!(adapter = %name, "Adapter context created");
                contexts.push(ctx);
            }
        }

        info!("Runtime initialized");

        Ok(())
    }

    /// Starts the runtime.
    pub async fn start(&self) -> RuntimeResult<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Runtime is already running");
                return Ok(());
            }
            *running = true;
        }

        info!("Starting Alloy runtime");

        // Start all adapters
        let adapter_names = self.adapter_names().await;
        let mut contexts = self.adapter_contexts.write().await;

        for (idx, name) in adapter_names.iter().enumerate() {
            if let Some(adapter) = self.get_adapter(name).await
                && let Some(ctx) = contexts.get_mut(idx)
            {
                if let Err(e) = adapter.on_start(ctx).await {
                    error!(adapter = %name, error = %e, "Failed to start adapter");
                    continue;
                }
                info!(adapter = %name, "Adapter started");
            }
        }

        let stats = self.stats().await;
        info!("Runtime started: {}", stats);

        Ok(())
    }

    /// Stops the runtime and all adapters.
    pub async fn stop(&self) -> RuntimeResult<()> {
        {
            let mut running = self.running.write().await;
            if !*running {
                warn!("Runtime is not running");
                return Ok(());
            }
            *running = false;
        }

        info!("Stopping Alloy runtime");

        // Call on_shutdown for all adapters
        let adapter_names = self.adapter_names().await;
        let mut contexts = self.adapter_contexts.write().await;

        for (i, name) in adapter_names.iter().enumerate() {
            if let Some(adapter) = self.get_adapter(name).await
                && let Some(ctx) = contexts.get_mut(i)
                && let Err(e) = adapter.on_shutdown(ctx).await
            {
                error!(adapter = %name, error = %e, "Error during adapter shutdown");
            }
        }

        info!("Runtime stopped");

        Ok(())
    }

    /// Runs the runtime until a shutdown signal is received.
    pub async fn run(&self) -> RuntimeResult<()> {
        self.init().await?;
        self.start().await?;

        info!("Alloy runtime is now running. Press Ctrl+C to stop.");

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        self.stop().await?;

        Ok(())
    }

    /// Runs the runtime with a custom shutdown future.
    pub async fn run_until<F>(&self, shutdown: F) -> RuntimeResult<()>
    where
        F: std::future::Future<Output = ()>,
    {
        self.init().await?;
        self.start().await?;

        shutdown.await;

        self.stop().await?;

        Ok(())
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

    /// Returns statistics about the runtime.
    pub async fn stats(&self) -> RuntimeStats {
        let adapters = self.adapters.read().await;
        let running = *self.running.read().await;

        RuntimeStats {
            running,
            adapter_count: adapters.len(),
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

// =============================================================================
// RuntimeStats
// =============================================================================

/// Statistics about the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Whether the runtime is running.
    pub running: bool,
    /// Number of registered adapters.
    pub adapter_count: usize,
}

impl std::fmt::Display for RuntimeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Runtime: {} | Adapters: {}",
            if self.running { "Running" } else { "Stopped" },
            self.adapter_count
        )
    }
}
