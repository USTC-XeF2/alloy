//! Milky protocol message types.
//!
//! Distinguishes incoming (received) and outgoing (sent) segments - each
//! direction has its own typed enum with precisely-typed fields.
//!
//! # [`IncomingSegment`]
//!
//! Received from the Milky server; only deserializable. Carries full
//! server-side metadata (resource IDs, temporary URLs, dimensions, etc.).
//! Includes segment types that can only be received, never sent:
//! `File`, `MarketFace`, `Xml`.
//!
//! # [`OutgoingSegment`]
//!
//! Sent to the Milky server; only serializable. Uses URIs for media upload
//! and omits server-side metadata. Implements all builder helper methods.

pub mod common;
pub mod incoming;
pub mod outgoing;

pub use incoming::IncomingSegment;
pub use outgoing::OutgoingSegment;
