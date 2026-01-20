//! Bot registry for managing dynamically connected bots.
//!
//! In the capability-based system, bots are registered dynamically when
//! connections are established and unregistered when they disconnect.

use crate::bot::{Bot, BotStatus, ManagedBot};
use alloy_core::{BotManager, BoxedAdapter, BoxedEvent, ConnectionHandle, Dispatcher};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Registry for managing adapters and dynamic bot instances.
pub struct BotRegistry {
    /// Map of bot ID to bot instance.
    bots: Arc<RwLock<HashMap<String, Arc<RwLock<Bot>>>>>,
    /// Map of adapter name to adapter instance.
    adapters: Arc<RwLock<HashMap<String, BoxedAdapter>>>,
    /// Shared dispatcher reference.
    dispatcher: Arc<RwLock<Option<Arc<RwLock<Dispatcher>>>>>,
}

impl BotRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            bots: Arc::new(RwLock::new(HashMap::new())),
            adapters: Arc::new(RwLock::new(HashMap::new())),
            dispatcher: Arc::new(RwLock::new(None)),
        }
    }

    /// Sets the dispatcher for event routing.
    pub async fn set_dispatcher(&self, dispatcher: Arc<RwLock<Dispatcher>>) {
        let mut d = self.dispatcher.write().await;
        *d = Some(dispatcher);
    }

    /// Gets the dispatcher reference.
    pub async fn get_dispatcher(&self) -> Option<Arc<RwLock<Dispatcher>>> {
        let d = self.dispatcher.read().await;
        d.clone()
    }

    /// Registers an adapter.
    pub async fn register_adapter(&self, adapter: BoxedAdapter) {
        let name = adapter.name().to_string();
        let mut adapters = self.adapters.write().await;
        adapters.insert(name.clone(), adapter);
        debug!(adapter = %name, "Registered adapter");
    }

    /// Gets an adapter by name.
    pub async fn get_adapter(&self, name: &str) -> Option<BoxedAdapter> {
        let adapters = self.adapters.read().await;
        adapters.get(name).cloned()
    }

    /// Returns all registered adapter names.
    pub async fn adapter_names(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Creates a BotManager that wraps this registry's event dispatching.
    pub fn create_bot_manager(&self) -> Arc<BotManager> {
        let dispatcher = Arc::clone(&self.dispatcher);

        Arc::new(BotManager::new(Arc::new(
            move |event: BoxedEvent, bot: alloy_core::BoxedBot| {
                let dispatcher = Arc::clone(&dispatcher);
                tokio::spawn(async move {
                    let guard = dispatcher.read().await;
                    if let Some(ref disp) = *guard {
                        let d = disp.read().await;
                        let _ = d.dispatch(event, bot).await;
                    }
                });
            },
        )))
    }

    /// Registers a new bot from a connection.
    ///
    /// This is called by connection handlers when a new connection is established.
    pub async fn register_bot(
        &self,
        id: String,
        adapter_name: String,
        connection: ConnectionHandle,
    ) -> anyhow::Result<Arc<RwLock<Bot>>> {
        let mut bots = self.bots.write().await;

        if bots.contains_key(&id) {
            anyhow::bail!("Bot with ID '{}' is already registered", id);
        }

        let mut bot = ManagedBot::new(id.clone(), adapter_name.clone(), connection);

        // Set dispatcher if available
        if let Some(dispatcher) = self.get_dispatcher().await {
            bot.set_dispatcher(dispatcher);
        }

        info!(bot_id = %id, adapter = %adapter_name, "Registered new bot");

        let bot_arc = Arc::new(RwLock::new(bot));
        bots.insert(id, Arc::clone(&bot_arc));

        Ok(bot_arc)
    }

    /// Unregisters a bot by ID.
    pub async fn unregister_bot(&self, id: &str) -> anyhow::Result<()> {
        let mut bots = self.bots.write().await;

        if let Some(bot) = bots.remove(id) {
            // Disconnect the bot
            let mut bot_guard = bot.write().await;
            if bot_guard.is_connected().await {
                bot_guard.disconnect().await;
            }
            info!(bot_id = %id, "Unregistered bot");
            Ok(())
        } else {
            anyhow::bail!("Bot with ID '{}' not found", id);
        }
    }

    /// Gets a reference to a bot by ID.
    pub async fn get(&self, id: &str) -> Option<Arc<RwLock<Bot>>> {
        let bots = self.bots.read().await;
        bots.get(id).cloned()
    }

    /// Returns all bot IDs.
    pub async fn ids(&self) -> Vec<String> {
        let bots = self.bots.read().await;
        bots.keys().cloned().collect()
    }

    /// Returns the number of registered bots.
    pub async fn count(&self) -> usize {
        let bots = self.bots.read().await;
        bots.len()
    }

    /// Disconnects all registered bots.
    pub async fn disconnect_all(&self) -> anyhow::Result<()> {
        let bots = self.bots.read().await;

        info!("Disconnecting {} bot(s)", bots.len());

        for (id, bot) in bots.iter() {
            let mut bot_guard = bot.write().await;
            if bot_guard.is_connected().await {
                bot_guard.disconnect().await;
                debug!(bot_id = %id, "Disconnected bot");
            }
        }

        Ok(())
    }

    /// Gets the status of all bots.
    pub async fn status_all(&self) -> HashMap<String, BotStatus> {
        let bots = self.bots.read().await;
        let mut statuses = HashMap::new();

        for (id, bot) in bots.iter() {
            let bot_guard = bot.read().await;
            statuses.insert(id.clone(), bot_guard.status().await);
        }

        statuses
    }

    /// Returns statistics about the registry.
    pub async fn stats(&self) -> RegistryStats {
        let bots = self.bots.read().await;
        let adapters = self.adapters.read().await;

        let mut connected = 0;
        let mut reconnecting = 0;
        let mut disconnected = 0;

        for bot in bots.values() {
            let bot_guard = bot.read().await;
            match bot_guard.status().await {
                BotStatus::Connected => connected += 1,
                BotStatus::Reconnecting => reconnecting += 1,
                BotStatus::Disconnected => disconnected += 1,
            }
        }

        RegistryStats {
            total: bots.len(),
            connected,
            reconnecting,
            disconnected,
            adapters: adapters.len(),
        }
    }
}

impl Default for BotRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the bot registry.
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Total number of bots.
    pub total: usize,
    /// Number of connected bots.
    pub connected: usize,
    /// Number of reconnecting bots.
    pub reconnecting: usize,
    /// Number of disconnected bots.
    pub disconnected: usize,
    /// Number of registered adapters.
    pub adapters: usize,
}

impl std::fmt::Display for RegistryStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Bots: {} total ({} connected, {} reconnecting, {} disconnected), {} adapters",
            self.total, self.connected, self.reconnecting, self.disconnected, self.adapters
        )
    }
}
