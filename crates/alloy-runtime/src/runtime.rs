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
use tracing::{debug, error, info, span, warn};

use crate::config::{AlloyConfig, ConfigLoader, ConfigResult};
use crate::error::{RuntimeError, RuntimeResult};
use crate::logging;
use alloy_core::{
    AdapterBridge, BoxedAdapter, BoxedBot, BoxedEvent, ConfigurableAdapter, Dispatcher,
    TransportContext,
};
use alloy_framework::{AlloyContext, Matcher};

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
    /// Registered matchers for event dispatching.
    matchers: Arc<RwLock<Vec<Matcher>>>,
    /// Transport context.
    transport_context: TransportContext,
    /// Adapter bridges (populated after init).
    adapter_bridges: Arc<RwLock<HashMap<String, Arc<AdapterBridge>>>>,
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
            matchers: Arc::new(RwLock::new(Vec::new())),
            transport_context: transport_ctx,
            adapter_bridges: Arc::new(RwLock::new(HashMap::new())),
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
        let config: A::Config = if let Some(config_value) = self.config.adapters.get(adapter_name) {
            // Deserialize from config
            config_value.clone().deserialize().map_err(|e| {
                RuntimeError::AdapterConfigDeserialize(format!(
                    "Failed to deserialize config for adapter '{adapter_name}': {e}"
                ))
            })?
        } else {
            // Use default configuration
            warn!(
                adapter = adapter_name,
                "No configuration found for adapter, using default"
            );
            Default::default()
        };

        // Create adapter from its config
        let adapter = A::from_config(config)?;

        let mut adapters = self.adapters.write().await;
        adapters.insert(adapter_name.to_string(), adapter);
        info!(adapter = adapter_name, "Registered adapter");
        Ok(())
    }

    /// Registers a matcher for event dispatching.
    ///
    /// Matchers are checked in the order they are added. Each matcher
    /// contains multiple handlers and a check rule.
    pub async fn register_matcher(&self, matcher: Matcher) {
        let mut matchers = self.matchers.write().await;
        matchers.push(matcher);
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
        let mut matcher_list = self.matchers.write().await;
        matcher_list.extend(matchers);
    }

    /// Returns the number of registered matchers.
    pub async fn matcher_count(&self) -> usize {
        self.matchers.read().await.len()
    }

    /// Returns whether the runtime is currently running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    // =========================================================================
    // Adapter Management
    // =========================================================================

    /// Creates an event dispatcher for adapters.
    ///
    /// This dispatcher function is called when events are received from the transport layer.
    /// It dispatches events to all registered matchers, executing their handlers and
    /// respecting blocking rules.
    fn create_event_dispatcher(&self) -> Dispatcher {
        let matchers = Arc::clone(&self.matchers);
        Arc::new(move |event: BoxedEvent, bot: BoxedBot| {
            let matchers = Arc::clone(&matchers);
            tokio::spawn(async move {
                let event_name = event.event_name();
                let span = span!(tracing::Level::DEBUG, "dispatch", event_name = %event_name);
                let _enter = span.enter();

                let ctx = Arc::new(AlloyContext::new(event, bot));
                let matcher_list = matchers.read().await;

                for matcher in matcher_list.iter() {
                    if matcher.execute(Arc::clone(&ctx)).await && matcher.is_blocking() {
                        debug!(
                            matcher = matcher.get_name().unwrap_or("unnamed"),
                            "Blocking matcher matched, stopping dispatch"
                        );
                        break;
                    }
                }
            });
        })
    }

    /// Initializes all registered adapters with transport capabilities.
    pub async fn init(&self) -> RuntimeResult<()> {
        let adapters = self.adapters.read().await;
        debug!("Initializing {} adapter(s)", adapters.len());

        let mut bridges = self.adapter_bridges.write().await;

        for (name, adapter) in adapters.iter() {
            // Create adapter bridge
            let bridge = Arc::new(AdapterBridge::new(
                adapter.clone(),
                self.create_event_dispatcher(),
                self.transport_context.clone(),
            ));

            debug!(adapter = %name, "Adapter bridge created");
            bridges.insert(name.clone(), bridge);
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
        let adapters = self.adapters.read().await;
        let bridges = self.adapter_bridges.read().await;

        for (name, adapter) in adapters.iter() {
            if let Some(bridge) = bridges.get(name) {
                if let Err(e) = adapter.on_start(bridge.clone()).await {
                    error!(adapter = %name, error = %e, "Failed to start adapter");
                    continue;
                }
                info!(adapter = %name, "Adapter started");
            }
        }

        info!("Runtime started");

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
        let adapters = self.adapters.read().await;
        let bridges = self.adapter_bridges.read().await;

        for (name, adapter) in adapters.iter() {
            if let Some(bridge) = bridges.get(name)
                && let Err(e) = adapter.on_shutdown(bridge.clone()).await
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
