//! Milky protocol adapter for the Alloy bot framework.
//!
//! This crate implements the Milky protocol v1.1, providing event parsing,
//! strongly-typed API methods, and transport management for QQ bot development.
//!
//! # Architecture
//!
//! - [`MilkyAdapter`] — Implements `Adapter` + `ConfigurableAdapter`, handles
//!   transport setup and message routing.
//! - [`MilkyBot`] — Implements `Bot`, provides strongly-typed Milky API methods.
//! - [`model`] — Protocol types: events, segments, messages, entities, API responses.
//!
//! # Supported Transports
//!
//! In the Milky protocol, API calls are **always HTTP** (`POST /api/{action}`).
//! The adapter sets up an HTTP client for every client-mode connection so that
//! API calls work regardless of how events are received.
//!
//! | Transport    | Events         | API calls    |
//! |--------------|----------------|-------------|
//! | `sse-client` | SSE `/event`   | HTTP `/api/*` |
//! | `ws-client`  | WS  `/event`   | HTTP `/api/*` |
//! | `http-server`| Webhook POST   | — (receive-only) |

mod adapter;
mod bot;

pub mod api;
pub mod config;
pub mod model;

pub use adapter::MilkyAdapter;
pub use bot::MilkyBot;
pub use config::MilkyConfig;
pub use model::event::*;
pub use model::message::{IncomingSegment, OutgoingSegment};
