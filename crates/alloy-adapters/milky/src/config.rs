//! Configuration types for the Milky adapter.
//!
//! This module defines the configuration schema loaded from the global
//! `alloy.toml` configuration file.
//!
//! # Milky Protocol Overview
//!
//! In the Milky protocol the **protocol end** (Milky server) always provides
//! two HTTP endpoints:
//!
//! - `POST /api/:action` — API calls sent by the application.
//! - `GET  /event`       — Event stream (SSE or WebSocket upgrade).
//!
//! Because API calls are **always HTTP**, there is no WebSocket-based API call
//! mechanism. Each connection type configures how the application receives
//! events and reaches the `/api/*` endpoint.
//!
//! # Connection Types
//!
//! - `type = "client"` + optional `event_source`:
//!   - `none` (default): API-only client
//!   - `sse`: subscribe to `/event` via SSE
//!   - `ws`: subscribe to `/event` via WebSocket
//! - `type = "webhook"`: receive-only HTTP webhook server

use serde::Deserialize;

/// Milky adapter configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MilkyConfig {
    /// List of connection configurations.
    #[serde(default)]
    pub connections: Vec<ConnectionConfig>,
}

/// Connection configuration for a single Milky connection.
///
/// Contains common fields and flattens the connection type specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConnectionConfig {
    /// Unified client mode.
    ///
    /// Always creates the HTTP API client. Event intake is controlled by
    /// [`event_source`].
    Client {
        /// Base HTTP URL of the Milky server (e.g. `http://127.0.0.1:8081`).
        url: String,

        /// Bearer token for authentication.
        #[serde(default)]
        access_token: Option<String>,

        /// Event source mode.
        #[serde(default)]
        #[serde(flatten)]
        event_source: EventSource,
    },

    /// Receive events pushed by the Milky server (webhook mode, receive-only).
    Webhook {
        /// Bind address (default: `127.0.0.1`).
        #[serde(default = "default_host")]
        host: String,

        /// Listen port.
        port: u16,

        /// Webhook path the Milky server should POST events to (default: `/`).
        #[serde(default = "default_webhook_path")]
        path: String,

        /// Bearer token for authentication validation.
        #[serde(default)]
        access_token: Option<String>,
    },
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_webhook_path() -> String {
    "/".into()
}

/// Event source mode for unified client connections.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "event_source", rename_all = "lowercase")]
pub enum EventSource {
    /// Do not subscribe to events; API-only mode.
    #[default]
    None,

    /// Subscribe to `/event` via SSE.
    Sse(#[serde(default)] ReconnectConfig),

    /// Subscribe to `/event` via WebSocket.
    Ws(#[serde(default)] ReconnectConfig),
}

/// Reconnection configuration for event intake.
#[derive(Debug, Clone, Deserialize)]
pub struct ReconnectConfig {
    /// Re-establish the event connection automatically on disconnect.
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,

    /// Initial delay (ms) before the first reconnection attempt.
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
}

const fn default_auto_reconnect() -> bool {
    true
}

const fn default_reconnect_delay_ms() -> u64 {
    3000
}
