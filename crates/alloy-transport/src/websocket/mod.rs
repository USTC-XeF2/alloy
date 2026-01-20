//! WebSocket transport capabilities.
//!
//! This module provides WebSocket client and server implementations.

#[cfg(feature = "ws-client")]
mod client;
#[cfg(feature = "ws-client")]
pub use client::WsClientCapabilityImpl;

#[cfg(feature = "ws-server")]
mod server;
#[cfg(feature = "ws-server")]
pub use server::WsServerCapabilityImpl;
