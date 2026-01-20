//! Main runtime orchestration with capability-based transport system.
//!
//! The runtime initializes adapters with a TransportContext containing
//! available transport capabilities. Adapters then use these capabilities
//! to establish connections dynamically.

use crate::bot::BotStatus;
use crate::logging::{LoggingBuilder, SpanEvents};
use crate::registry::{BotRegistry, RegistryStats};
use alloy_core::{AdapterContext, BoxedAdapter, Dispatcher, Matcher, TransportContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{Level, debug, error, info, warn};

/// Global flag to track if logging has been initialized.
static LOGGING_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// The main Alloy runtime that orchestrates adapters and bots.
///
/// # Capability-Based Architecture
///
/// The runtime provides transport capabilities to adapters during initialization:
///
/// ```rust,ignore
/// let runtime = AlloyRuntime::new();
///
/// // Register adapters
/// runtime.register_adapter(OneBotAdapter::new()).await;
///
/// // Register matchers with handlers
/// runtime.register_matcher(
///     Matcher::new()
///         .on::<MessageEvent>()
///         .handler(echo_handler)
/// ).await;
///
/// // Run - adapters will discover transport capabilities and set up connections
/// runtime.run().await?;
/// ```
pub struct AlloyRuntime {
    /// The bot registry.
    registry: Arc<BotRegistry>,
    /// The event dispatcher.
    dispatcher: Arc<RwLock<Dispatcher>>,
    /// Transport context (populated before init).
    transport_context: Arc<RwLock<Option<TransportContext>>>,
    /// Adapter contexts (populated after init).
    adapter_contexts: Arc<RwLock<Vec<AdapterContext>>>,
    /// Whether the runtime is running.
    running: Arc<RwLock<bool>>,
    /// Log level (for reference).
    #[allow(dead_code)]
    log_level: String,
}

impl AlloyRuntime {
    /// Creates a new runtime with default settings.
    ///
    /// This automatically initializes logging with default settings (INFO level)
    /// and creates a TransportContext with all available transport capabilities.
    pub fn new() -> Self {
        Self::init_logging_default();

        // Create transport context with all available capabilities
        let transport_ctx = Self::create_default_transport_context();

        Self {
            registry: Arc::new(BotRegistry::new()),
            dispatcher: Arc::new(RwLock::new(Dispatcher::new())),
            transport_context: Arc::new(RwLock::new(Some(transport_ctx))),
            adapter_contexts: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            log_level: "info".to_string(),
        }
    }

    /// Creates a new runtime with a specific log level.
    ///
    /// This also creates a TransportContext with all available transport capabilities.
    pub fn with_log_level(log_level: impl Into<String>) -> Self {
        let level_str = log_level.into();
        Self::init_logging_with_level(&level_str);

        // Create transport context with all available capabilities
        let transport_ctx = Self::create_default_transport_context();

        Self {
            registry: Arc::new(BotRegistry::new()),
            dispatcher: Arc::new(RwLock::new(Dispatcher::new())),
            transport_context: Arc::new(RwLock::new(Some(transport_ctx))),
            adapter_contexts: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            log_level: level_str,
        }
    }

    /// Initializes logging with default settings.
    fn init_logging_default() {
        if LOGGING_INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            LoggingBuilder::new()
                .with_level(Level::INFO)
                .with_span_events(SpanEvents::NONE)
                .init();
        }
    }

    /// Creates a default TransportContext with all available transport capabilities.
    ///
    /// This method automatically registers all transport implementations based on
    /// enabled cargo features.
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

    /// Initializes logging with a specific level.
    fn init_logging_with_level(level_str: &str) {
        if LOGGING_INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let level = match level_str.to_lowercase().as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" | "warning" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            };

            // Use lifecycle span events for debug/trace, none for others
            let span_events = if matches!(level, Level::TRACE | Level::DEBUG) {
                SpanEvents::LIFECYCLE
            } else {
                SpanEvents::NONE
            };

            LoggingBuilder::new()
                .with_level(level)
                .with_span_events(span_events)
                .init();

            info!(level = %level_str, "Logging initialized");
        }
    }

    /// Returns whether logging has been initialized.
    pub fn is_logging_initialized() -> bool {
        LOGGING_INITIALIZED.load(Ordering::SeqCst)
    }

    /// Manually initializes logging with custom settings.
    ///
    /// This should be called BEFORE creating an `AlloyRuntime` if you want
    /// custom logging configuration.
    pub fn init_logging_custom<F>(init_fn: F)
    where
        F: FnOnce(),
    {
        if LOGGING_INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            init_fn();
        }
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
    /// Adapters must be registered before calling `init()` or `run()`.
    pub async fn register_adapter(&self, adapter: BoxedAdapter) {
        self.registry.register_adapter(adapter).await;
    }

    /// Registers a matcher with the dispatcher.
    ///
    /// Matchers are checked in the order they are added. Each matcher
    /// contains multiple handlers and a check rule.
    pub async fn register_matcher(&self, matcher: Matcher) {
        let mut dispatcher = self.dispatcher.write().await;
        dispatcher.add(matcher);
    }

    /// Returns a reference to the bot registry.
    pub fn registry(&self) -> &Arc<BotRegistry> {
        &self.registry
    }

    /// Returns a reference to the dispatcher.
    pub fn dispatcher(&self) -> &Arc<RwLock<Dispatcher>> {
        &self.dispatcher
    }

    /// Returns whether the runtime is currently running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Initializes all registered adapters with transport capabilities.
    pub async fn init(&self) -> anyhow::Result<()> {
        // Set dispatcher in registry
        self.registry
            .set_dispatcher(Arc::clone(&self.dispatcher))
            .await;

        // Get transport context (use empty if not set)
        let transport_ctx = {
            let guard = self.transport_context.read().await;
            guard.clone().unwrap_or_default()
        };

        let adapter_names = self.registry.adapter_names().await;
        debug!("Initializing {} adapter(s)", adapter_names.len());

        let mut contexts = self.adapter_contexts.write().await;

        for name in adapter_names {
            if let Some(adapter) = self.registry.get_adapter(&name).await {
                // Create bot manager for this adapter
                let bot_manager = self.registry.create_bot_manager();

                // Create adapter context
                let mut ctx = AdapterContext::new(transport_ctx.clone(), bot_manager);

                // Call adapter's on_init
                if let Err(e) = adapter.on_init(&mut ctx).await {
                    error!(adapter = %name, error = %e, "Failed to initialize adapter");
                    continue;
                }

                debug!(adapter = %name, "Adapter initialized");
                contexts.push(ctx);
            }
        }

        info!("Runtime initialized");

        Ok(())
    }

    /// Starts the runtime.
    pub async fn start(&self) -> anyhow::Result<()> {
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
        let adapter_names = self.registry.adapter_names().await;
        let mut contexts = self.adapter_contexts.write().await;

        for (idx, name) in adapter_names.iter().enumerate() {
            if let Some(adapter) = self.registry.get_adapter(name).await
                && let Some(ctx) = contexts.get_mut(idx)
            {
                if let Err(e) = adapter.on_start(ctx).await {
                    error!(adapter = %name, error = %e, "Failed to start adapter");
                    continue;
                }
                info!(adapter = %name, "Adapter started");
            }
        }

        let stats = self.registry.stats().await;
        info!("Runtime started: {}", stats);

        Ok(())
    }

    /// Stops the runtime and all adapters.
    pub async fn stop(&self) -> anyhow::Result<()> {
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
        let adapter_names = self.registry.adapter_names().await;
        let mut contexts = self.adapter_contexts.write().await;

        for (i, name) in adapter_names.iter().enumerate() {
            if let Some(adapter) = self.registry.get_adapter(name).await
                && let Some(ctx) = contexts.get_mut(i)
                && let Err(e) = adapter.on_shutdown(ctx).await
            {
                error!(adapter = %name, error = %e, "Error during adapter shutdown");
            }
        }

        // Disconnect all bots
        self.registry.disconnect_all().await?;

        info!("Runtime stopped");

        Ok(())
    }

    /// Runs the runtime until a shutdown signal is received.
    pub async fn run(&self) -> anyhow::Result<()> {
        self.init().await?;
        self.start().await?;

        info!("Alloy runtime is now running. Press Ctrl+C to stop.");

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        self.stop().await?;

        Ok(())
    }

    /// Runs the runtime with a custom shutdown future.
    pub async fn run_until<F>(&self, shutdown: F) -> anyhow::Result<()>
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
        let registry_stats = self.registry.stats().await;
        let running = *self.running.read().await;

        RuntimeStats {
            running,
            registry: registry_stats,
        }
    }

    /// Gets the status of a specific bot.
    pub async fn bot_status(&self, id: &str) -> Option<BotStatus> {
        if let Some(bot) = self.registry.get(id).await {
            let guard = bot.read().await;
            Some(guard.status().await)
        } else {
            None
        }
    }
}

impl Default for AlloyRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Whether the runtime is running.
    pub running: bool,
    /// Registry statistics.
    pub registry: RegistryStats,
}

impl std::fmt::Display for RuntimeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Runtime: {}, {}",
            if self.running { "Running" } else { "Stopped" },
            self.registry
        )
    }
}
