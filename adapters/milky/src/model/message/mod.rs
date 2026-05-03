//! Milky protocol message types: [`IncomingSegment`] (received) and [`OutgoingSegment`] (sent).
//!
//! Each direction has its own typed enum with protocol-specific fields.

pub mod common;
pub mod incoming;
pub mod outgoing;

pub use incoming::IncomingSegment;
pub use outgoing::OutgoingSegment;
