//! Transport capability system for the Alloy framework.
//!
//! This module provides a capability-based approach for adapters to discover
//! and use available transport features at runtime.
//!
//! # Overview
//!
//! Each capability is a plain function pointer (`fn(Args...) -> BoxFuture`).
//! Because they carry no captured state, all parameters are passed explicitly.
//! Adapters call them via `ctx.ws_client()(url, handler, config).await`.
//!
//! # Dynamic Bot Management
//!
//! Bots can join/leave at runtime:
//! - **Server transports**: New connections become bots, disconnections remove them
//! - **Client transports**: Configured endpoints auto-reconnect on disconnect

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use linkme::distributed_slice;
use tracing::warn;

use super::config::{HttpClientConfig, WsClientConfig};
use super::connection::{ConnectionHandle, ConnectionInfo, ListenerHandle};
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
    fn get_bot_id(&self, conn_info: ConnectionInfo) -> TransportResult<String>;

    /// Create and register a bot for a new connection.
    fn create_bot(&self, bot_id: &str, connection: ConnectionHandle);

    /// Process incoming data from a connection.
    async fn on_message(&self, bot_id: &str, data: &[u8]);

    /// Called when a connection is closed.
    async fn on_disconnect(&self, bot_id: &str);
}

// =============================================================================
// Capability Function Types
// =============================================================================

/// Function pointer that starts a WebSocket server listener.
///
/// Parameters: `(addr, path, handler)` — all owned to satisfy `'static` bounds.
pub type WsListenFn = fn(
    String,
    String,
    Arc<dyn ConnectionHandler>,
) -> BoxFuture<'static, TransportResult<ListenerHandle>>;

/// Function pointer that opens a WebSocket client connection.
///
/// Parameters: `(config, handler)`.
pub type WsConnectFn = fn(
    WsClientConfig,
    Arc<dyn ConnectionHandler>,
) -> BoxFuture<'static, TransportResult<ConnectionHandle>>;

/// Function pointer that starts an HTTP server listener.
///
/// Parameters: `(addr, path, handler)`.
pub type HttpListenFn = fn(
    String,
    String,
    Arc<dyn ConnectionHandler>,
) -> BoxFuture<'static, TransportResult<ListenerHandle>>;

/// Function pointer that registers an HTTP outbound API-client bot.
///
/// Parameters: `(bot_id, config, handler)`.
pub type HttpStartClientFn = fn(
    String,
    HttpClientConfig,
    Arc<dyn ConnectionHandler>,
) -> BoxFuture<'static, TransportResult<ConnectionHandle>>;

// =============================================================================
// Capability Registries (linkme distributed slices)
// =============================================================================

/// Registry of WebSocket server listen function pointers.
/// Each crate that provides a ws-server capability contributes one entry.
#[distributed_slice]
pub static WS_LISTEN_REGISTRY: [WsListenFn];

/// Registry of WebSocket client connect function pointers.
#[distributed_slice]
pub static WS_CONNECT_REGISTRY: [WsConnectFn];

/// Registry of HTTP server listen function pointers.
#[distributed_slice]
pub static HTTP_LISTEN_REGISTRY: [HttpListenFn];

/// Registry of HTTP client start function pointers.
#[distributed_slice]
pub static HTTP_START_CLIENT_REGISTRY: [HttpStartClientFn];

// Will be defined as impl method for TransportContext

// =============================================================================
// Transport Context
// =============================================================================

/// Context for adapter initialization.
///
/// Provides access to available transport capabilities.
#[derive(Copy, Clone)]
pub struct TransportContext {
    ws_server: Option<WsListenFn>,
    ws_client: Option<WsConnectFn>,
    http_server: Option<HttpListenFn>,
    http_client: Option<HttpStartClientFn>,
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

    /// Builds a [`TransportContext`] from all capability functions registered via
    /// `#[register_capability(...)]`.
    ///
    /// If multiple providers are registered for the same capability type a warning
    /// is emitted and the **first** one wins.
    pub fn collect_all() -> Self {
        fn load<T: Copy>(registry: &[T], name: &str) -> Option<T> {
            match registry.len() {
                0 => None,
                1 => Some(registry[0]),
                n => {
                    warn!(
                        count = n,
                        capability = name,
                        "Multiple capability providers registered, using first"
                    );
                    Some(registry[0])
                }
            }
        }

        TransportContext {
            ws_server: load(&WS_LISTEN_REGISTRY, "ws_server"),
            ws_client: load(&WS_CONNECT_REGISTRY, "ws_client"),
            http_server: load(&HTTP_LISTEN_REGISTRY, "http_server"),
            http_client: load(&HTTP_START_CLIENT_REGISTRY, "http_client"),
        }
    }

    /// Registers the WebSocket server capability.
    pub fn with_ws_server(mut self, f: WsListenFn) -> Self {
        self.ws_server = Some(f);
        self
    }

    /// Registers the WebSocket client capability.
    pub fn with_ws_client(mut self, f: WsConnectFn) -> Self {
        self.ws_client = Some(f);
        self
    }

    /// Registers the HTTP server capability.
    pub fn with_http_server(mut self, f: HttpListenFn) -> Self {
        self.http_server = Some(f);
        self
    }

    /// Registers the HTTP client capability.
    pub fn with_http_client(mut self, f: HttpStartClientFn) -> Self {
        self.http_client = Some(f);
        self
    }

    /// Gets the WebSocket server capability if available.
    pub fn ws_server(&self) -> Option<WsListenFn> {
        self.ws_server
    }

    /// Gets the WebSocket client capability if available.
    pub fn ws_client(&self) -> Option<WsConnectFn> {
        self.ws_client
    }

    /// Gets the HTTP server capability if available.
    pub fn http_server(&self) -> Option<HttpListenFn> {
        self.http_server
    }

    /// Gets the HTTP client capability if available.
    pub fn http_client(&self) -> Option<HttpStartClientFn> {
        self.http_client
    }
}

impl Default for TransportContext {
    fn default() -> Self {
        Self::new()
    }
}
