//! Transport capability system for the Alloy framework.
//!
//! This module provides a capability-based approach for adapters to discover
//! and use available transport features at runtime.
//!
//! # Overview
//!
//! Each capability is a plain function pointer (`fn(Args...) -> BoxFuture`).
//! Because they carry no captured state, all parameters are passed explicitly.
//! Adapters call them via `ctx.ws_client()(config, handler, resolve_bot_id).await`.
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
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::config::{
    HttpClientConfig, HttpServerConfig, SseClientConfig, WsClientConfig, WsServerConfig,
};
use super::connection::{ConnectionInfo, ListenerHandle, PostJsonFn, Sender};
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
    /// Idempotently registers a bot and its optional send-capable connection sender.
    ///
    /// ## Behaviour
    /// - **Bot absent**: creates the bot via `adapter.create_bot`, stores a new
    ///   [`ConnectionHandle`] with `sender`, and returns a fresh [`CancellationToken`]
    ///   bound to that handle.
    /// - **Bot present, `sender` is `None`**: returns the existing handle's token
    ///   unchanged (no-op).
    /// - **Bot present, existing `sender` is `None`**: upgrades the handle's sender to
    ///   the supplied `sender` and returns the existing token.
    /// - **Bot present, existing `sender` is `Some`**: keeps the existing sender
    ///   and returns the existing token (the bot already has send capability).
    ///
    /// The returned token is the one stored in the bot's [`ConnectionHandle`].
    /// Transport loops should listen on it for graceful shutdown.
    fn register_connection(&self, bot_id: &str, sender: Option<Sender>) -> CancellationToken;

    /// Process incoming data from a connection.
    async fn on_message(&self, bot_id: &str, data: &[u8]);

    /// Called when a connection is closed.
    async fn on_disconnect(&self, bot_id: &str);

    /// Register a listener handle, keeping it alive for the adapter's lifetime.
    fn add_listener(&self, handle: ListenerHandle);
}

// =============================================================================
// Capability Function Types
// =============================================================================

/// Synchronous bot-id resolver used by server transports.
pub type ServerBotIdFn = Arc<dyn Fn(ConnectionInfo) -> Option<String> + Send + Sync>;

/// Asynchronous bot-id resolver used by client transports.
pub type ClientBotIdFn =
    Arc<dyn Fn(PostJsonFn) -> BoxFuture<'static, Option<String>> + Send + Sync>;

/// Function pointer that starts a WebSocket server listener.
///
/// Parameters: `(config, handler, resolve_bot_id)` — config contains bind address,
/// port, and path.
pub type WsListenFn = fn(
    WsServerConfig,
    Arc<dyn ConnectionHandler>,
    ServerBotIdFn,
) -> BoxFuture<'static, TransportResult<()>>;

/// Function pointer that opens a WebSocket client connection.
///
/// Parameters: `(config, handler, resolve_bot_id)`.
pub type WsConnectFn = fn(
    WsClientConfig,
    Arc<dyn ConnectionHandler>,
    String,
) -> BoxFuture<'static, TransportResult<String>>;

/// Function pointer that starts an HTTP server listener.
///
/// Parameters: `(config, handler, resolve_bot_id)` — config contains bind address,
/// port, and path.
pub type HttpListenFn = fn(
    HttpServerConfig,
    Arc<dyn ConnectionHandler>,
    ServerBotIdFn,
) -> BoxFuture<'static, TransportResult<()>>;

/// Function pointer that registers an HTTP outbound API-client bot.
///
/// Parameters: `(config, handler, resolve_bot_id)` — config contains connection settings.
pub type HttpStartClientFn = fn(
    HttpClientConfig,
    Arc<dyn ConnectionHandler>,
    ClientBotIdFn,
) -> BoxFuture<'static, TransportResult<String>>;

/// Function pointer that opens a persistent SSE client connection.
///
/// Parameters: `(config, handler, bot_id)` — config contains connection settings.
pub type SseClientFn = fn(
    SseClientConfig,
    Arc<dyn ConnectionHandler>,
    String,
) -> BoxFuture<'static, TransportResult<String>>;

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

/// Registry of SSE client function pointers.
#[distributed_slice]
pub static SSE_CLIENT_REGISTRY: [SseClientFn];

// Will be defined as impl method for TransportContext

// =============================================================================
// Transport Context
// =============================================================================

/// Context for adapter initialization.
///
/// Provides access to available transport capabilities.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportContext {
    ws_server: Option<WsListenFn>,
    ws_client: Option<WsConnectFn>,
    http_server: Option<HttpListenFn>,
    http_client: Option<HttpStartClientFn>,
    sse_client: Option<SseClientFn>,
}

impl TransportContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self {
            ws_server: None,
            ws_client: None,
            http_server: None,
            http_client: None,
            sse_client: None,
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
            sse_client: load(&SSE_CLIENT_REGISTRY, "sse_client"),
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

    /// Registers the SSE client capability.
    pub fn with_sse_client(mut self, f: SseClientFn) -> Self {
        self.sse_client = Some(f);
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

    /// Gets the SSE client capability if available.
    pub fn sse_client(&self) -> Option<SseClientFn> {
        self.sse_client
    }
}
