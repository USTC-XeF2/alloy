//! # Alloy Transport
//!
//! Network transport capability implementations for the Alloy bot framework.
//!
//! This crate provides concrete implementations of transport capabilities registered through
//! `#[register_capability(...)]` attribute macros. These implementations are automatically
//! discovered and collected by `TransportContext::collect_all()` at runtime.
//!
//! ## Features
//!
//! - `ws-client`: WebSocket client capability
//! - `ws-server`: WebSocket server capability
//! - `http-client`: HTTP client capability
//! - `http-server`: HTTP server capability
//! - `full`: All capabilities
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Adapter Layer (OneBot, Discord, etc.)  │
//! │  Calls: ws_connect(), http_start_client(), etc.
//! ├─────────────────────────────────────────┤
//! │  alloy-core TransportContext            │
//! │  Holds: Arc<dyn Fn(...) -> BoxFuture>   │
//! ├─────────────────────────────────────────┤
//! │  alloy-transport async fn implementations
//! │  Registered via #[register_capability] macro
//! ├─────────────────────────────────────────┤
//! │  Network Layer (TCP/HTTP/WebSocket)     │
//! └─────────────────────────────────────────┘
//! ```
//!
//! ## Capability Implementations
//!
//! | Function | Feature | Request Type | Response Type |
//! |----------|---------|--------------|---------------|
//! | `ws_connect()` | `ws-client` | `(url, handler, config)` | `ConnectionHandle` |
//! | `ws_listen()` | `ws-server` | `(addr, path, handler)` | `ListenerHandle` |
//! | `http_start_client()` | `http-client` | `(bot_id, api_url, token, handler)` | `ConnectionHandle` |
//! | `http_listen()` | `http-server` | `(addr, path, handler)` | `ListenerHandle` |
//!
//! All capabilities are automatically discovered via `linkme::distributed_slice` registration
//! and collected into a [`TransportContext`] at startup.

// Transport implementations (feature-gated)

// ─── Unified server module (all server logic: infrastructure + impls) ────────
#[cfg(any(feature = "http-server", feature = "ws-server"))]
mod server;

// ─── Root-level client implementations ─────────────────────────────────────
#[cfg(feature = "http-client")]
mod http_client;

#[cfg(feature = "ws-client")]
mod ws_client;

// ─── Capability re-exports ───────────────────────────────────────────────────
// Server capabilities (all from crate::server module)
#[cfg(feature = "http-server")]
pub use server::http_listen;

#[cfg(feature = "ws-server")]
pub use server::ws_listen;

// Client capabilities (from root-level modules)
#[cfg(feature = "http-client")]
pub use http_client::http_start_client;

#[cfg(feature = "ws-client")]
pub use ws_client::ws_connect;
