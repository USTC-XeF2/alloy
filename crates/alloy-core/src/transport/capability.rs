//! Transport capability system for the Alloy framework.
//!
//! This module provides a capability-based approach for adapters to discover
//! and use available transport features at runtime.
//!
//! # Overview
//!
//! Instead of configuring transports upfront, adapters query available capabilities
//! and register handlers for the ones they need:
//!
//! ```rust,ignore
//! impl Adapter for MyAdapter {
//!     fn on_init(&self, ctx: &TransportContext) {
//!         // Try to get WebSocket server capability
//!         if let Some(ws_server) = ctx.get_capability::<dyn WsServerCapability>() {
//!             ws_server.listen("0.0.0.0:8080", self.message_handler());
//!         }
//!
//!         // Try to get WebSocket client capability  
//!         if let Some(ws_client) = ctx.get_capability::<dyn WsClientCapability>() {
//!             ws_client.connect("ws://127.0.0.1:9000", self.message_handler());
//!         }
//!     }
//! }
//! ```
//!
//! # Dynamic Bot Management
//!
//! Bots can join/leave at runtime:
//! - **Server transports**: New connections become bots, disconnections remove them
//! - **Client transports**: Configured endpoints auto-reconnect on disconnect

use std::sync::Arc;

use async_trait::async_trait;

use super::connection::{ClientConfig, ConnectionHandle, ConnectionInfo, ListenerHandle};
use crate::error::TransportResult;

// =============================================================================
// Connection Handler
// =============================================================================

/// Interface for handling connection lifecycle events from transport implementations.
///
/// When a transport connection is established, data arrives, or a connection closes,
/// the transport layer calls methods on this handler to drive the bot lifecycle.
///
/// [`AdapterBridge`](crate::adapter::AdapterBridge) is the built-in implementation.
#[async_trait]
pub trait ConnectionHandler: Send + Sync {
    /// Extract a bot ID from connection metadata when a new connection arrives.
    async fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String>;

    /// Create and register a bot for a new connection.
    async fn create_bot(&self, bot_id: &str, connection: ConnectionHandle);

    /// Process incoming data from a connection.
    async fn on_message(&self, bot_id: &str, data: &[u8]);

    /// Called when a connection is closed.
    async fn on_disconnect(&self, bot_id: &str);
}

// =============================================================================
// Transport Capabilities
// =============================================================================

/// WebSocket server capability.
///
/// Allows an adapter to listen for incoming WebSocket connections.
#[async_trait]
pub trait WsServerCapability: Send + Sync {
    /// Starts listening on the specified address.
    ///
    /// Each incoming connection will be handled by the provided handler.
    /// Returns a handle that can be used to stop the listener.
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ListenerHandle>;
}

/// WebSocket client capability.
///
/// Allows an adapter to connect to WebSocket servers.
#[async_trait]
pub trait WsClientCapability: Send + Sync {
    /// Connects to a WebSocket server.
    ///
    /// The connection will auto-reconnect based on the retry config.
    /// Returns a handle that can be used to manage the connection.
    async fn connect(
        &self,
        url: &str,
        handler: Arc<dyn ConnectionHandler>,
        config: ClientConfig,
    ) -> TransportResult<ConnectionHandle>;
}

/// HTTP server capability.
///
/// Allows an adapter to receive HTTP callbacks.
#[async_trait]
pub trait HttpServerCapability: Send + Sync {
    /// Starts an HTTP server on the specified address.
    ///
    /// The handler will be called for each incoming request.
    async fn listen(
        &self,
        addr: &str,
        path: &str,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ListenerHandle>;
}

/// HTTP client capability.
///
/// Allows an adapter to make HTTP requests.
#[async_trait]
pub trait HttpClientCapability: Send + Sync {
    /// Starts an HTTP client bot.
    ///
    /// Creates a bot that can send API calls via HTTP POST.
    /// The bot is registered with the provided handler and can be used
    /// to send messages, but will not receive events through this connection.
    async fn start_client(
        &self,
        bot_id: &str,
        api_url: &str,
        access_token: Option<String>,
        handler: Arc<dyn ConnectionHandler>,
    ) -> TransportResult<ConnectionHandle>;
}

// =============================================================================
// Transport Context
// =============================================================================

/// Context for adapter initialization.
///
/// Provides access to available transport capabilities.
#[derive(Clone)]
pub struct TransportContext {
    ws_server: Option<Arc<dyn WsServerCapability>>,
    ws_client: Option<Arc<dyn WsClientCapability>>,
    http_server: Option<Arc<dyn HttpServerCapability>>,
    http_client: Option<Arc<dyn HttpClientCapability>>,
}

impl TransportContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self {
            ws_server: None,
            ws_client: None,
            http_server: None,
            http_client: None,
        }
    }

    /// Registers the WebSocket server capability.
    pub fn with_ws_server(mut self, cap: Arc<dyn WsServerCapability>) -> Self {
        self.ws_server = Some(cap);
        self
    }

    /// Registers the WebSocket client capability.
    pub fn with_ws_client(mut self, cap: Arc<dyn WsClientCapability>) -> Self {
        self.ws_client = Some(cap);
        self
    }

    /// Registers the HTTP server capability.
    pub fn with_http_server(mut self, cap: Arc<dyn HttpServerCapability>) -> Self {
        self.http_server = Some(cap);
        self
    }

    /// Registers the HTTP client capability.
    pub fn with_http_client(mut self, cap: Arc<dyn HttpClientCapability>) -> Self {
        self.http_client = Some(cap);
        self
    }

    /// Sets the WebSocket server capability.
    pub fn set_ws_server(&mut self, cap: Arc<dyn WsServerCapability>) {
        self.ws_server = Some(cap);
    }

    /// Sets the WebSocket client capability.
    pub fn set_ws_client(&mut self, cap: Arc<dyn WsClientCapability>) {
        self.ws_client = Some(cap);
    }

    /// Sets the HTTP server capability.
    pub fn set_http_server(&mut self, cap: Arc<dyn HttpServerCapability>) {
        self.http_server = Some(cap);
    }

    /// Sets the HTTP client capability.
    pub fn set_http_client(&mut self, cap: Arc<dyn HttpClientCapability>) {
        self.http_client = Some(cap);
    }

    /// Gets the WebSocket server capability if available.
    pub fn ws_server(&self) -> Option<&Arc<dyn WsServerCapability>> {
        self.ws_server.as_ref()
    }

    /// Gets the WebSocket client capability if available.
    pub fn ws_client(&self) -> Option<&Arc<dyn WsClientCapability>> {
        self.ws_client.as_ref()
    }

    /// Gets the HTTP server capability if available.
    pub fn http_server(&self) -> Option<&Arc<dyn HttpServerCapability>> {
        self.http_server.as_ref()
    }

    /// Gets the HTTP client capability if available.
    pub fn http_client(&self) -> Option<&Arc<dyn HttpClientCapability>> {
        self.http_client.as_ref()
    }

    /// Checks if WebSocket server is available.
    pub fn has_ws_server(&self) -> bool {
        self.ws_server.is_some()
    }

    /// Checks if WebSocket client is available.
    pub fn has_ws_client(&self) -> bool {
        self.ws_client.is_some()
    }

    /// Checks if HTTP server is available.
    pub fn has_http_server(&self) -> bool {
        self.http_server.is_some()
    }

    /// Checks if HTTP client is available.
    pub fn has_http_client(&self) -> bool {
        self.http_client.is_some()
    }
}

impl Default for TransportContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_context() {
        let ctx = TransportContext::new();
        assert!(!ctx.has_ws_server());
        assert!(!ctx.has_ws_client());
        assert!(!ctx.has_http_server());
        assert!(!ctx.has_http_client());
    }
}
