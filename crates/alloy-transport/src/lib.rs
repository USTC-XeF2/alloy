//! # Alloy Transport
//!
//! Network transport capability implementations for the Alloy bot framework.
//!
//! This crate provides concrete implementations of transport capabilities defined in `alloy-core`.
//! It supports multiple transport types through feature flags.
//!
//! ## Features
//!
//! - `ws-client` (default): WebSocket client capability
//! - `ws-server`: WebSocket server capability
//! - `http-client`: HTTP client capability
//! - `http-server`: HTTP server capability
//! - `all-clients`: Both client capabilities
//! - `all-servers`: Both server capabilities
//! - `full`: All capabilities
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │  Adapter Layer      │  (OneBot, Discord, etc.)
//! │  (uses capabilities)│
//! ├─────────────────────┤
//! │  alloy-core         │  (capability traits)
//! ├─────────────────────┤
//! │  alloy-transport    │  <- This crate (implementations)
//! ├─────────────────────┤
//! │  Network (TCP/HTTP) │
//! └─────────────────────┘
//! ```
//!
//! ## Capability Implementations
//!
//! | Capability | Description | Use Case |
//! |------------|-------------|----------|
//! | `WsClientCapability` | WebSocket client | Connect to bot backend |
//! | `WsServerCapability` | WebSocket server | Accept reverse WebSocket |
//! | `HttpClientCapability` | HTTP client | Make API requests |
//! | `HttpServerCapability` | HTTP server | Receive event callbacks |
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use alloy_transport::websocket::WsClientCapabilityImpl;
//! use alloy_core::{WsClientCapability, ClientConfig};
//!
//! // Create capability implementation
//! let capability = WsClientCapabilityImpl::new();
//!
//! // Use with adapter
//! let config = ClientConfig::default();
//! let handle = capability.connect("ws://127.0.0.1:8080", handler, config).await?;
//! ```
//!
//! ```rust,ignore
//! use alloy_transport::websocket::{WsServerConfig, WsServerTransport};
//! use alloy_transport::traits::{Transport, ServerTransport};
//!
//! let config = WsServerConfig::new("0.0.0.0", 9000);
//! let transport = WsServerTransport::new(config);
//! transport.start().await?;
//!
//! // Handle connections
//! let mut rx = transport.take_message_receiver().await.unwrap();
//! while let Some(msg) = rx.recv().await {
//!     match msg {
//!         TransportMessage::Connected { conn_id } => {
//!             println!("Client connected: {}", conn_id);
//!         }
//!         TransportMessage::Received { conn_id, data } => {
//!             // Echo back
//!             transport.send(&conn_id, data).await?;
//!         }
//!         _ => {}
//!     }
//! }
//! ```

// Transport implementations (feature-gated)
#[cfg(any(feature = "http-client", feature = "http-server"))]
pub mod http;

#[cfg(any(feature = "ws-client", feature = "ws-server"))]
pub mod websocket;

// Capability re-exports
#[cfg(feature = "ws-server")]
pub use websocket::WsServerCapabilityImpl;

#[cfg(feature = "ws-client")]
pub use websocket::WsClientCapabilityImpl;

#[cfg(feature = "http-server")]
pub use http::HttpServerCapabilityImpl;

#[cfg(feature = "http-client")]
pub use http::HttpClientCapabilityImpl;
