//! Runtime bot instance management.
//!
//! This module provides [`BotInstance`] which wraps a connection and manages
//! its lifecycle. Bot instances can join/leave dynamically at runtime.
//!
//! Note: [`BotInstance`] is a runtime management type, different from
//! `alloy_core::Bot` which is a trait defining bot capabilities.

use std::sync::Arc;

use alloy_core::{ConnectionHandle, TransportResult};
use alloy_framework::Dispatcher;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Represents the current status of a bot instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotStatus {
    /// Bot is connected and running.
    Connected,
    /// Bot is reconnecting after a disconnection.
    Reconnecting,
    /// Bot has been disconnected.
    Disconnected,
}

impl std::fmt::Display for BotStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "Connected"),
            Self::Reconnecting => write!(f, "Reconnecting"),
            Self::Disconnected => write!(f, "Disconnected"),
        }
    }
}

/// A runtime bot instance that represents an active connection.
///
/// In the capability-based system, bot instances are created dynamically
/// when connections are established (server) or connected (client).
///
/// This is different from `alloy_core::Bot` trait which defines bot
/// capabilities - `BotInstance` manages the runtime lifecycle.
pub struct BotInstance {
    /// Unique identifier (from connection handler).
    id: String,
    /// Adapter name.
    adapter_name: String,
    /// Current status.
    status: Arc<RwLock<BotStatus>>,
    /// Connection handle for sending messages.
    connection: ConnectionHandle,
    /// Dispatcher for event routing.
    dispatcher: Option<Arc<RwLock<Dispatcher>>>,
}

impl BotInstance {
    /// Creates a new bot instance from a connection.
    pub fn new(
        id: impl Into<String>,
        adapter_name: impl Into<String>,
        connection: ConnectionHandle,
    ) -> Self {
        Self {
            id: id.into(),
            adapter_name: adapter_name.into(),
            status: Arc::new(RwLock::new(BotStatus::Connected)),
            connection,
            dispatcher: None,
        }
    }

    /// Sets the dispatcher for event routing.
    pub fn set_dispatcher(&mut self, dispatcher: Arc<RwLock<Dispatcher>>) {
        self.dispatcher = Some(dispatcher);
    }

    /// Returns the bot's unique identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the adapter name.
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Returns the current status.
    pub async fn status(&self) -> BotStatus {
        *self.status.read().await
    }

    /// Returns whether the bot is currently connected.
    pub async fn is_connected(&self) -> bool {
        matches!(self.status().await, BotStatus::Connected)
    }

    /// Returns a clone of the connection handle.
    pub fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }

    /// Sets the bot's status.
    pub(crate) async fn set_status(&self, status: BotStatus) {
        let mut guard = self.status.write().await;
        let old_status = *guard;
        *guard = status;
        debug!(
            bot_id = %self.id,
            old_status = %old_status,
            new_status = %status,
            "Bot status changed"
        );
    }

    /// Sends data through the connection.
    pub async fn send(&self, data: Vec<u8>) -> TransportResult<()> {
        self.connection.send(data).await
    }

    /// Sends a JSON value through the connection.
    pub async fn send_json(&self, value: &serde_json::Value) -> TransportResult<()> {
        self.connection.send_json(value).await
    }

    /// Disconnects this bot.
    pub async fn disconnect(&mut self) {
        self.connection.close();
        self.set_status(BotStatus::Disconnected).await;
        info!(bot_id = %self.id, "Bot disconnected");
    }
}

impl std::fmt::Debug for BotInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotInstance")
            .field("id", &self.id)
            .field("adapter_name", &self.adapter_name)
            .finish()
    }
}
