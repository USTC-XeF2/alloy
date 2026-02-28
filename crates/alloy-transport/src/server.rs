//! Unified server module for HTTP and WebSocket server transports.
//!
//! This module consolidates all server-side transport logic:
//! - **Shared infrastructure**: TCP listener management, global registry, routing dispatch
//! - **HTTP server**: [`HttpServerCapabilityImpl`] for POST-based event reception
//! - **WebSocket server**: [`WsServerCapabilityImpl`] for WebSocket reverse connections
//!
//! ## Architecture
//!
//! Both HTTP and WebSocket servers ultimately bind TCP sockets. This module:
//! 1. Maintains a **global registry** (`SERVER_REGISTRY`) mapping each bind
//!    address to a live [`ServerEntry`].
//! 2. Binds the TCP socket **once per address**, serving a single axum [`Router`]
//!    that dispatches requests dynamically to registered route handlers.
//! 3. Automatically shuts down when the last route is unregistered (Arc/Weak-based lifecycle).
//!
//! Multiple adapters can listen on the same address with different paths:
//! ```text
//! 0.0.0.0:8080
//! ├── GET  /ws       → WsRouteHandler (OneBot reverse WebSocket)
//! ├── POST /webhook  → HttpRouteHandler (Adapter A events)
//! └── POST /events   → HttpRouteHandler (Adapter B events)
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock, Weak};

use axum::{
    Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
};
use parking_lot::{Mutex, RwLock};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use alloy_core::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, ListenerHandle, TransportResult,
};
use alloy_macros::register_capability;

#[cfg(feature = "http-server")]
use axum::{body::Bytes, response::Response, routing::post};

#[cfg(feature = "ws-server")]
use {
    axum::{
        extract::{
            WebSocketUpgrade,
            ws::{Message, WebSocket},
        },
        routing::get,
    },
    futures::{SinkExt, StreamExt},
};

// ─── Route handlers (concrete types, no trait objects) ────────────────────────

/// Handles all POST events for a single registered HTTP path.
///
/// Maintains a table of known bots so that each `bot_id` is only initialised
/// once over the lifetime of the listener.
#[cfg(feature = "http-server")]
struct HttpBotHandler {
    handler: Arc<dyn ConnectionHandler>,
    known_bots: Mutex<HashMap<String, ConnectionHandle>>,
}

/// Handles all WebSocket connections arriving at a single registered path.
///
/// Maintains an active-connection table (bot_id → send channel) so that the
/// adapter's [`ConnectionHandler`] can address individual bots by ID.
#[cfg(feature = "ws-server")]
struct WsBotHandler {
    handler: Arc<dyn ConnectionHandler>,
    /// Active connections: bot_id → sender half of the outgoing message channel.
    connections: Mutex<HashMap<String, mpsc::Sender<Vec<u8>>>>,
}

// ─── Shared runtime state (one per bound address) ───────────────────────────────

/// Runtime state shared between the axum handler and the registration helpers.
///
/// Fields are conditionally compiled so that, for example, a build with only
/// `ws-server` enabled never allocates or references the `http_routes` map.
struct SharedState {
    /// HTTP route table: path → handler.
    #[cfg(feature = "http-server")]
    http_routes: RwLock<HashMap<String, Arc<HttpBotHandler>>>,

    /// WebSocket route table: path → handler.
    #[cfg(feature = "ws-server")]
    ws_routes: RwLock<HashMap<String, Arc<WsBotHandler>>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            #[cfg(feature = "http-server")]
            http_routes: RwLock::new(HashMap::new()),
            #[cfg(feature = "ws-server")]
            ws_routes: RwLock::new(HashMap::new()),
        }
    }
}

// ─── Server lifecycle container ───────────────────────────────────────────────

/// One entry per bound address in [`SERVER_REGISTRY`].
///
/// The server stops when the last `Arc<ServerEntry>` clone is dropped (each
/// registered route holds one clone; deregistration drops it).
struct ServerEntry {
    /// The actual bind address resolved by the OS (includes ephemeral port).
    actual_addr: String,
    /// Route tables and other shared axum state.
    state: Arc<SharedState>,
    /// Cancellation token for graceful shutdown.
    shutdown_token: CancellationToken,
}

impl Drop for ServerEntry {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
    }
}

// ─── Global registry ──────────────────────────────────────────────────────────

static SERVER_REGISTRY: LazyLock<Mutex<HashMap<String, Weak<ServerEntry>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ─── Public entry point ───────────────────────────────────────────────────────

/// Returns the live [`ServerEntry`] for `addr`, creating one if needed.
///
/// The first call for a given address binds the TCP listener and spawns the
/// axum serve loop.  Subsequent calls for the same address (while the first
/// `Arc<ServerEntry>` is still live) re-use the existing server; only route
/// table entries are added.
async fn get_or_create_server(addr: &str) -> std::io::Result<Arc<ServerEntry>> {
    // ── Fast path: server already exists ──────────────────────────────────────
    {
        let registry = SERVER_REGISTRY.lock();
        if let Some(weak) = registry.get(addr)
            && let Some(entry) = weak.upgrade()
        {
            return Ok(entry);
        }
    }

    // ── Slow path: bind a new listener and start serving ──────────────────────
    let state = Arc::new(SharedState::new());
    let listener = TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    let actual_addr_str = actual_addr.to_string();

    let router = build_router(state.clone());
    let shutdown_token = CancellationToken::new();

    let entry = Arc::new(ServerEntry {
        actual_addr: actual_addr_str.clone(),
        state: state.clone(),
        shutdown_token: shutdown_token.clone(),
    });

    // Store a weak reference so the registry does not prevent cleanup.
    {
        let mut registry = SERVER_REGISTRY.lock();
        registry.insert(addr.to_string(), Arc::downgrade(&entry));
    }

    debug!(addr = %actual_addr, "Shared TCP server started");

    tokio::spawn(async move {
        let server = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        );
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    error!(error = %e, "Shared server error");
                }
            }
            () = shutdown_token.cancelled() => {
                info!(addr = %actual_addr, "Shared server shutting down");
            }
        }
    });

    Ok(entry)
}

// ─── Router construction ──────────────────────────────────────────────────────

/// Builds the axum [`Router`] for this server.
///
/// Routes are added conditionally:
/// * `GET  /{*path}` and `GET  /` → [`ws_dispatch`]    (only with `ws-server`)
/// * `POST /{*path}` and `POST /` → [`http_dispatch`]  (only with `http-server`)
///
/// A fallback returns **404** for any method/path combination that has no
/// registered handler.
fn build_router(state: Arc<SharedState>) -> Router {
    let mut router = Router::new();

    // ── HTTP POST ─────────────────────────────────────────────────────────────
    #[cfg(feature = "http-server")]
    {
        router = router
            .route("/{*path}", post(http_dispatch))
            .route("/", post(http_dispatch));
    }

    // ── WebSocket (GET + Upgrade) ──────────────────────────────────────────────
    #[cfg(feature = "ws-server")]
    {
        router = router
            .route("/{*path}", get(ws_dispatch))
            .route("/", get(ws_dispatch));
    }

    router.with_state(state)
}

// ─── HTTP dispatch ────────────────────────────────────────────────────────────

/// Axum handler for HTTP POST requests.
///
/// Looks up the request path in `SharedState::http_routes` and delegates to
/// the registered [`HttpRouteHandler`], or returns **404** if none is found.
#[cfg(feature = "http-server")]
async fn http_dispatch(
    State(state): State<Arc<SharedState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let handler = state.http_routes.read().get(&path).cloned();

    match handler {
        Some(h) => h.handle(addr, headers, body).await,
        None => (
            StatusCode::NOT_FOUND,
            format!("No HTTP handler for path: {path}"),
        )
            .into_response(),
    }
}

// ─── WebSocket dispatch ───────────────────────────────────────────────────────

/// Axum handler for WebSocket upgrade requests.
///
/// Looks up the request path in `SharedState::ws_routes` and upgrades the
/// connection, delegating the socket to the registered [`WsRouteHandler`].
/// Non-WebSocket GET requests (without the upgrade header) receive **404**.
#[cfg(feature = "ws-server")]
async fn ws_dispatch(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SharedState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    uri: Uri,
    headers: HeaderMap,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let handler = state.ws_routes.read().get(&path).cloned();

    match handler {
        Some(h) => {
            // Collect headers as a plain map (lowercase keys) before the move.
            let metadata: HashMap<String, String> = headers
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|v| (name.as_str().to_lowercase(), v.to_string()))
                })
                .collect();

            debug!(remote_addr = %addr, path = %path, "New WebSocket connection request");
            ws.on_upgrade(move |socket| async move {
                h.handle(addr, metadata, socket).await;
            })
            .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            format!("No WebSocket handler for path: {path}"),
        )
            .into_response(),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// HTTP SERVER CAPABILITY IMPLEMENTATION
// ═════════════════════════════════════════════════════════════════════════════

/// Starts (or re-uses) a TCP server on `addr` and registers a POST handler
/// for `path`.
///
/// Multiple calls with the **same `addr` but different `path`** values will
/// share one TCP listener; the shared dispatcher routes each request to the
/// correct handler.
///
/// This function is registered as the `HttpListenFn` capability.
#[cfg(feature = "http-server")]
#[register_capability(http_server)]
pub async fn http_listen(
    addr: String,
    path: String,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<ListenerHandle> {
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };

    let entry = get_or_create_server(&addr).await?;
    info!(
        addr = %entry.actual_addr,
        path = %path,
        "HTTP server listening",
    );

    let route_handler = Arc::new(HttpBotHandler {
        handler,
        known_bots: Mutex::new(HashMap::new()),
    });
    entry
        .state
        .http_routes
        .write()
        .insert(path.clone(), route_handler);
    info!(path = %path, addr = %entry.actual_addr, "Registered HTTP route");

    let handle_id = format!("http-server-{}{}", entry.actual_addr, path);
    let shutdown_token = CancellationToken::new();
    let token_clone = shutdown_token.clone();

    tokio::spawn(async move {
        token_clone.cancelled().await;
        entry.state.http_routes.write().remove(&path);
        info!(path = %path, "Unregistered HTTP route");
    });

    Ok(ListenerHandle::new(handle_id, shutdown_token))
}

#[cfg(feature = "http-server")]
impl HttpBotHandler {
    /// Handles an HTTP POST request from a bot.
    async fn handle(&self, addr: SocketAddr, headers: HeaderMap, body: Bytes) -> Response {
        // Build connection info from request headers.
        let mut conn_info = ConnectionInfo::new("http").with_remote_addr(addr.to_string());
        for (name, value) in &headers {
            if let Ok(v) = value.to_str() {
                conn_info = conn_info.with_metadata(name.as_str().to_lowercase(), v.to_string());
            }
        }

        // Ask the adapter to identify which bot this request belongs to.
        let bot_id = match self.handler.get_bot_id(conn_info) {
            Ok(id) => id,
            Err(e) => {
                error!(
                    error       = %e,
                    remote_addr = %addr,
                    "Failed to extract bot ID from HTTP request",
                );
                return (StatusCode::BAD_REQUEST, "Failed to extract bot ID").into_response();
            }
        };

        // First request from this bot → create the bot and its outgoing channel.
        {
            let mut known = self.known_bots.lock();
            if !known.contains_key(&bot_id) {
                let (message_tx, mut message_rx) = mpsc::channel::<Vec<u8>>(256);
                let shutdown_token = CancellationToken::new();

                let connection_handle =
                    ConnectionHandle::new_http_server(bot_id.clone(), message_tx, shutdown_token);

                // Drain the outgoing queue; adapters can plug in SSE or similar
                // by overriding this logic in their own transport wrapper.
                let bot_id_clone = bot_id.clone();
                tokio::spawn(async move {
                    while let Some(data) = message_rx.recv().await {
                        warn!(
                            bot_id = %bot_id_clone,
                            len    = data.len(),
                            "HTTP bot sent message but no push mechanism configured \
                             (consider SSE)",
                        );
                    }
                });

                self.handler.create_bot(&bot_id, connection_handle.clone());
                known.insert(bot_id.clone(), connection_handle);
                info!(bot_id = %bot_id, remote_addr = %addr, "HTTP bot created");
            }
        }

        debug!(bot_id = %bot_id, len = body.len(), "Received HTTP POST");
        self.handler.on_message(&bot_id, &body).await;

        (StatusCode::OK, "ok").into_response()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// WEBSOCKET SERVER CAPABILITY IMPLEMENTATION
// ═════════════════════════════════════════════════════════════════════════════

/// Starts (or re-uses) a TCP server on `addr` and registers a WebSocket
/// upgrade handler for `path`.
///
/// Multiple calls with the **same `addr` but different `path`** values share
/// one TCP listener; the dispatcher routes each request to the correct handler.
///
/// This function is registered as the `WsListenFn` capability.
#[cfg(feature = "ws-server")]
#[register_capability(ws_server)]
pub async fn ws_listen(
    addr: String,
    path: String,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<ListenerHandle> {
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };

    let entry = get_or_create_server(&addr).await?;
    info!(
        addr = %entry.actual_addr,
        path = %path,
        "WebSocket server listening",
    );

    let route_handler = Arc::new(WsBotHandler {
        handler,
        connections: Mutex::new(HashMap::new()),
    });
    entry
        .state
        .ws_routes
        .write()
        .insert(path.clone(), route_handler);
    info!(path = %path, addr = %entry.actual_addr, "Registered WebSocket route");

    let handle_id = format!("ws-server-{}{}", entry.actual_addr, path);
    let shutdown_token = CancellationToken::new();
    let token_clone = shutdown_token.clone();

    tokio::spawn(async move {
        token_clone.cancelled().await;
        entry.state.ws_routes.write().remove(&path);
        info!(path = %path, "Unregistered WebSocket route");
    });

    Ok(ListenerHandle::new(handle_id, shutdown_token))
}

#[cfg(feature = "ws-server")]
impl WsBotHandler {
    /// Handles a WebSocket upgrade and manages the connection lifecycle.
    async fn handle(&self, addr: SocketAddr, headers: HashMap<String, String>, socket: WebSocket) {
        let (mut ws_tx, mut ws_rx) = socket.split();

        // Build connection info from the HTTP upgrade headers.
        let mut conn_info = ConnectionInfo::new("websocket").with_remote_addr(addr.to_string());
        for (key, value) in &headers {
            conn_info = conn_info.with_metadata(key.clone(), value.clone());
        }

        // Let the adapter identify which bot this connection belongs to.
        let bot_id = match self.handler.get_bot_id(conn_info) {
            Ok(id) => id,
            Err(e) => {
                error!(
                    error       = %e,
                    remote_addr = %addr,
                    "Failed to establish WebSocket connection",
                );
                let _ = ws_tx.close().await;
                return;
            }
        };

        info!(bot_id = %bot_id, remote_addr = %addr, "WebSocket connection established");

        // Per-connection outgoing channel: adapter writes here → forwarded to ws_tx.
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);
        let shutdown_token = CancellationToken::new();
        let connection_handle =
            ConnectionHandle::new_ws(bot_id.clone(), tx.clone(), shutdown_token);

        self.handler.create_bot(&bot_id, connection_handle);
        self.connections.lock().insert(bot_id.clone(), tx.clone());

        // ── Send task: forwards outgoing frames to the WebSocket write half ───────
        let bot_id_send = bot_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                let text = String::from_utf8_lossy(&data).to_string();
                if ws_tx.send(Message::Text(text.into())).await.is_err() {
                    warn!(bot_id = %bot_id_send, "Failed to send message, connection closed");
                    break;
                }
            }
        });

        // ── Receive loop: forwards inbound frames to the adapter ─────────────────
        let handler_ref = self.handler.clone();
        let bot_id_recv = bot_id.clone();
        while let Some(result) = ws_rx.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    debug!(bot_id = %bot_id_recv, len = text.len(), "Received text message");
                    handler_ref.on_message(&bot_id_recv, text.as_bytes()).await;
                }
                Ok(Message::Binary(data)) => {
                    debug!(bot_id = %bot_id_recv, len = data.len(), "Received binary message");
                    handler_ref.on_message(&bot_id_recv, &data).await;
                }
                Ok(Message::Ping(_)) => {
                    debug!(bot_id = %bot_id_recv, "Received ping");
                }
                Ok(Message::Pong(_)) => {
                    debug!(bot_id = %bot_id_recv, "Received pong");
                }
                Ok(Message::Close(_)) => {
                    info!(bot_id = %bot_id_recv, "WebSocket connection closed by client");
                    break;
                }
                Err(e) => {
                    warn!(bot_id = %bot_id_recv, error = %e, "WebSocket error");
                    break;
                }
            }
        }

        // ── Cleanup ───────────────────────────────────────────────────────────────
        send_task.abort();
        self.connections.lock().remove(&bot_id);
        self.handler.on_disconnect(&bot_id).await;
        info!(bot_id = %bot_id, "WebSocket connection closed");
    }
}
