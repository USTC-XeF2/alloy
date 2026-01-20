//! Foundation layer - Core abstractions and type system.
//!
//! This module contains the fundamental building blocks of the Alloy framework:
//! - Event system for type-erased event passing
//! - Context management for event propagation
//! - Message abstractions for cross-protocol communication
//! - Unified error types

pub mod context;
pub mod error;
pub mod event;
pub mod message;

pub use context::AlloyContext;
pub use error::{
    AdapterError, AdapterResult, ExtractError, ExtractResult, TransportError, TransportResult,
};
pub use event::{BoxedEvent, Event, EventContext, FromEvent};
pub use message::{Message, MessageSegment};
