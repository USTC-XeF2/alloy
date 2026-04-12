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
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
};
use parking_lot::Mutex;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use alloy_core::error::TransportResult;
use alloy_core::transport::{ConnectionHandler, ConnectionInfo, ListenerHandle, ServerBotIdFn};
use alloy_macros::register_capability;

#[cfg(feature = "http-server")]
use axum::{response::Response, routing::post};

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
    tokio::sync::mpsc,
};

type RouteHandler = (Arc<dyn ConnectionHandler>, ServerBotIdFn);

#[derive(Default)]
struct SharedState {
    /// HTTP route table: path → connection handler.
    #[cfg(feature = "http-server")]
    http_routes: Mutex<HashMap<String, RouteHandler>>,

    /// WebSocket route table: path → connection handler.
    #[cfg(feature = "ws-server")]
    ws_routes: Mutex<HashMap<String, RouteHandler>>,
}

// ─── Server lifecycle container ───────────────────────────────────────────────

/// The server stops when the last `Arc<ServerEntry>` clone is dropped (each
/// registered route holds one clone; deregistration drops it).
struct ServerEntry {
    /// The actual bind address resolved by the OS (includes ephemeral port).
    actual_addr: SocketAddr,
    /// Route tables and other shared axum state.
    state: Arc<SharedState>,
    /// Cancellation receiver for graceful shutdown. Dropped on drop.
    _shutdown_rx: watch::Receiver<()>,
}

// ─── Global registry ──────────────────────────────────────────────────────────

static SERVER_REGISTRY: LazyLock<Mutex<HashMap<String, Weak<ServerEntry>>>> =
    LazyLock::new(Mutex::default);

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
    let state = Arc::new(SharedState::default());
    let listener = TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    let router = build_router(state.clone());
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    let entry = Arc::new(ServerEntry {
        actual_addr,
        state: state.clone(),
        _shutdown_rx: shutdown_rx,
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
        )
        .with_graceful_shutdown(async move {
            shutdown_tx.closed().await;
        });

        if let Err(e) = server.await {
            error!(error = %e, "Shared server error");
        }

        debug!(addr = %actual_addr, "Shared server shutting down");
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
    let path = uri.path();
    let entry = state.http_routes.lock().get(path).cloned();

    match entry {
        Some((handler, resolve_bot_id)) => {
            handle_http_request(handler, resolve_bot_id, addr, headers, body).await
        }
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
    let path = uri.path();
    let entry = state.ws_routes.lock().get(path).cloned();

    match entry {
        Some((handler, resolve_bot_id)) => {
            debug!(remote_addr = %addr, path = %path, "New WebSocket connection request");
            ws.on_upgrade(async move |socket| {
                handle_ws_connection(handler, resolve_bot_id, addr, headers, socket).await;
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

/// Starts (or re-uses) a TCP server and registers a POST handler.
///
/// Multiple calls with the **same bind address but different paths** will
/// share one TCP listener; the shared dispatcher routes each request to the
/// correct handler.
///
/// The resulting [`ListenerHandle`] is registered directly on `handler` via
/// [`ConnectionHandler::add_listener`].
///
/// This function is registered as the `HttpListenFn` capability.
#[cfg(feature = "http-server")]
#[register_capability(http_server)]
pub async fn http_listen(
    config: alloy_core::transport::HttpServerConfig,
    handler: Arc<dyn ConnectionHandler>,
    resolve_bot_id: ServerBotIdFn,
) -> TransportResult<()> {
    let path = if config.path.starts_with('/') {
        config.path.clone()
    } else {
        format!("/{}", config.path)
    };

    let addr = format!("{}:{}", config.bind_addr, config.port);
    let entry = get_or_create_server(&addr).await?;

    entry
        .state
        .http_routes
        .lock()
        .insert(path.clone(), (handler.clone(), resolve_bot_id));
    info!(path = %path, addr = %entry.actual_addr, "Registered HTTP route");

    let handle_id = format!("http-server-{}{}", entry.actual_addr, path);
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    tokio::spawn(async move {
        shutdown_tx.closed().await;
        entry.state.http_routes.lock().remove(&path);
        info!(path = %path, "Unregistered HTTP route");
    });

    handler.add_listener(ListenerHandle::new(handle_id, shutdown_rx));
    Ok(())
}

/// Handles a single HTTP POST request from a bot.
///
/// Resolves the bot ID, idempotently creates the bot, then forwards the body
/// to [`ConnectionHandler::on_message`].
#[cfg(feature = "http-server")]
async fn handle_http_request(
    handler: Arc<dyn ConnectionHandler>,
    resolve_bot_id: ServerBotIdFn,
    addr: SocketAddr,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Resolve which bot this request belongs to.
    let Some(bot_id) = resolve_bot_id(
        ConnectionInfo::new(addr)
            .with_headers(headers)
            .with_body(body.clone()),
    ) else {
        warn!(
            remote_addr = %addr,
            "Failed to extract bot ID from HTTP request metadata, cannot process request",
        );
        return (StatusCode::BAD_REQUEST, "Failed to extract bot ID").into_response();
    };

    // First request from this bot → register it (idempotent; no send capability for HTTP server).
    handler.register_connection(&bot_id, None);

    debug!(bot_id = %bot_id, len = body.len(), "Received HTTP POST");
    handler.on_message(&bot_id, body).await;

    (StatusCode::OK, "ok").into_response()
}

// ═════════════════════════════════════════════════════════════════════════════
// WEBSOCKET SERVER CAPABILITY IMPLEMENTATION
// ═════════════════════════════════════════════════════════════════════════════

/// Starts (or re-uses) a TCP server and registers a WebSocket upgrade handler.
///
/// Multiple calls with the **same bind address but different paths** share
/// one TCP listener; the dispatcher routes each request to the correct handler.
///
/// This function is registered as the `WsListenFn` capability.
#[cfg(feature = "ws-server")]
#[register_capability(ws_server)]
pub async fn ws_listen(
    config: alloy_core::transport::WsServerConfig,
    handler: Arc<dyn ConnectionHandler>,
    resolve_bot_id: ServerBotIdFn,
) -> TransportResult<()> {
    let path = if config.path.starts_with('/') {
        config.path.clone()
    } else {
        format!("/{}", config.path)
    };

    let addr = format!("{}:{}", config.bind_addr, config.port);
    let entry = get_or_create_server(&addr).await?;

    entry
        .state
        .ws_routes
        .lock()
        .insert(path.clone(), (handler.clone(), resolve_bot_id));
    info!(path = %path, addr = %entry.actual_addr, "Registered WebSocket route");

    let handle_id = format!("ws-server-{}{}", entry.actual_addr, path);
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    tokio::spawn(async move {
        shutdown_tx.closed().await;
        entry.state.ws_routes.lock().remove(&path);
        info!(path = %path, "Unregistered WebSocket route");
    });

    handler.add_listener(ListenerHandle::new(handle_id, shutdown_rx));
    Ok(())
}

/// Handles a single WebSocket connection for a bot.
///
/// Resolves the bot ID, registers the bot idempotently, then drives the
/// send/receive loop until the connection closes.
#[cfg(feature = "ws-server")]
async fn handle_ws_connection(
    handler: Arc<dyn ConnectionHandler>,
    resolve_bot_id: ServerBotIdFn,
    addr: SocketAddr,
    headers: HeaderMap,
    socket: WebSocket,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Resolve which bot this connection belongs to.
    let Some(bot_id) = resolve_bot_id(ConnectionInfo::new(addr).with_headers(headers)) else {
        warn!(
            remote_addr = %addr,
            "Failed to extract bot ID from WebSocket connection metadata, closing connection",
        );
        let _ = ws_tx.close().await;
        return;
    };

    info!(bot_id = %bot_id, remote_addr = %addr, "WebSocket connection established");

    // Per-connection outgoing channel: adapter writes here → forwarded to ws_tx.
    let (tx, mut rx) = mpsc::channel::<Bytes>(256);

    // Register the bot (or update its sender to Ws if it was previously receive-only).
    // The returned token drives graceful shutdown for this connection.
    let shutdown_token = handler.register_connection(
        &bot_id,
        Some(alloy_core::transport::Sender::Ws {
            message_tx: tx.clone(),
        }),
    );

    // ── Send task: forwards outgoing frames to the WebSocket write half ───────
    let bot_id_send = bot_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_tx.send(Message::Binary(data)).await.is_err() {
                warn!(bot_id = %bot_id_send, "Failed to send message, connection closed");
                break;
            }
        }
    });

    // ── Receive loop: forwards inbound frames to the adapter ─────────────────
    let bot_id_recv = bot_id.clone();
    loop {
        tokio::select! {
            // Graceful shutdown: bridge/adapter dropped the receiver.
            () = shutdown_token.closed() => {
                info!(bot_id = %bot_id_recv, "WebSocket connection shutting down");
                break;
            }
            result = ws_rx.next() => {
                match result {
                    Some(Ok(Message::Text(text))) => {
                        debug!(bot_id = %bot_id_recv, len = text.len(), "Received text message");
                        handler.on_message(&bot_id_recv, text.into()).await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        debug!(bot_id = %bot_id_recv, len = data.len(), "Received binary message");
                        handler.on_message(&bot_id_recv, data).await;
                    }
                    Some(Ok(Message::Ping(_))) => {
                        debug!(bot_id = %bot_id_recv, "Received ping");
                    }
                    Some(Ok(Message::Pong(_))) => {
                        debug!(bot_id = %bot_id_recv, "Received pong");
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!(bot_id = %bot_id_recv, "WebSocket connection closed by client");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!(bot_id = %bot_id_recv, error = %e, "WebSocket error");
                        break;
                    }
                }
            }
        }
    }

    // ── Cleanup ───────────────────────────────────────────────────────────────
    send_task.abort();
    handler.on_disconnect(&bot_id).await;
}
