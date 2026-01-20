//! HTTP transport capabilities.
//!
//! This module provides HTTP client and server implementations.

#[cfg(feature = "http-client")]
mod client;
#[cfg(feature = "http-client")]
pub use client::HttpClientCapabilityImpl;

#[cfg(feature = "http-server")]
mod server;
#[cfg(feature = "http-server")]
pub use server::HttpServerCapabilityImpl;
