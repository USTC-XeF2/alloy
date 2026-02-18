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
use std::sync::{Arc, RwLock};

use tokio::signal;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, info, span, warn};

use crate::config::{AlloyConfig, ConfigLoader, ConfigResult};
use crate::error::{RuntimeError, RuntimeResult};
use crate::logging;
use alloy_core::{
    AdapterBridge, BoxedBot, BoxedEvent, ConfigurableAdapter, Dispatcher, TransportContext,
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
/// runtime.run().await;
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
    /// Registered matchers for event dispatching.
    matchers: Arc<AsyncRwLock<Vec<Matcher>>>,
    /// Transport context.
    transport_context: TransportContext,
    /// Adapter bridges, created eagerly on registration.
    bridges: RwLock<HashMap<String, Arc<AdapterBridge>>>,
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

        // Create transport context with all available capabilities
        let transport_ctx = Self::create_default_transport_context();

        info!(
            log_level = %config.logging.level,
            log_format = ?config.logging.format,
            "Runtime initialized from configuration"
        );

        Self {
            config: config.clone(),
            matchers: Arc::new(AsyncRwLock::new(Vec::new())),
            transport_context: transport_ctx,
            bridges: RwLock::new(HashMap::new()),
            running: AtomicBool::new(false),
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
            self.create_event_dispatcher(),
            self.transport_context.clone(),
        ));

        self.bridges
            .write()
            .unwrap()
            .insert(adapter_name.to_string(), bridge);
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
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

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

        let bridges = self.bridges.read().unwrap();
        for (name, bridge) in bridges.iter() {
            if let Err(e) = bridge.on_start().await {
                error!(adapter = %name, error = %e, "Failed to start adapter");
            } else {
                info!(adapter = %name, "Adapter started");
            }
        }

        info!("Runtime started");
    }

    /// Stops the runtime and all adapters.
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

        let bridges = self.bridges.read().unwrap();
        for (name, bridge) in bridges.iter() {
            if let Err(e) = bridge.on_shutdown().await {
                error!(adapter = %name, error = %e, "Error during adapter shutdown");
            }
        }

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
