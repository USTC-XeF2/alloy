//! Event system for the Alloy framework.
//!
//! This module defines the EventView-based event model:
//!
//! - [`EventRoot`] for type-erased dispatch (`Arc<dyn EventRoot>`)
//! - [`EventView`] for typed extraction (`Event<T>` where `T: EventView`)
//! - [`EventType`] and [`Scene`] shared classification types

use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;

use downcast_rs::{Downcast, impl_downcast};

use super::message::RichTextSegment;

// ============================================================================
// Session Scene Identifier
// ============================================================================

/// Session scene (conversation context) identifier.
///
/// Returned by [`EventRoot::scene`](EventRoot::scene).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scene {
    Private {
        /// The user the bot is chatting with.
        user_id: String,
    },
    Group {
        /// The group / chat room the event came from.
        group_id: String,
        /// The sender inside the group, if available.
        user_id: Option<String>,
    },
    Guild {
        /// The guild or server this event belongs to.
        guild_id: String,
    },
    Other {
        /// A platform-specific identifier for the scene.
        id: String,
    },
}

// ============================================================================
// Event Type Classification
// ============================================================================

/// Classification of event types.
///
/// This enum represents the high-level category of an event, which is useful
/// for filtering events in matchers without knowing the specific event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Message events (private messages, group messages, etc.)
    Message,
    /// Notice events (group changes, recalls, friend adds, etc.)
    Notice,
    /// Request events (friend requests, group join requests, etc.)
    Request,
    /// Meta events (lifecycle, heartbeat, etc.)
    Meta,
    /// Other/unknown event types
    Other,
}

impl FromStr for EventType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "message" => EventType::Message,
            "notice" => EventType::Notice,
            "request" => EventType::Request,
            "meta" => EventType::Meta,
            _ => EventType::Other,
        })
    }
}

// ============================================================================
// EventRoot
// ============================================================================

/// Type-erased root event trait used by runtime dispatch.
pub trait EventRoot: Downcast + Send + Sync {
    /// Platform name (adapter name).
    fn platform(&self) -> &'static str;

    /// Adapter-specific event id suffix, e.g. `message.private.friend`.
    fn event_id(&self) -> String;

    /// High-level event classification.
    fn event_type(&self) -> EventType;

    /// Scene associated with this event if available.
    fn scene(&self) -> Option<Scene>;

    /// User id associated with this event if available.
    fn user_id(&self) -> Option<String> {
        match self.scene() {
            Some(Scene::Private { user_id }) => Some(user_id.clone()),
            Some(Scene::Group { user_id, .. }) => user_id,
            _ => None,
        }
    }

    /// Plain text projection of this event.
    fn plain_text(&self) -> String;

    /// Rich text projection of this event.
    fn rich_text(&self) -> Vec<RichTextSegment>;
}

impl_downcast!(EventRoot);

impl std::fmt::Debug for dyn EventRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventRoot")
            .field("id", &self.event_id())
            .field("platform", &self.platform())
            .field("type", &self.event_type())
            .finish()
    }
}

// ============================================================================
// EventView
// ============================================================================

/// Typed event view extraction trait.
pub trait EventView: Sized + Send {
    type Root: EventRoot;
    type Parent: EventView<Root = Self::Root>;

    fn from_root(event: Self::Root) -> Option<Self>;

    fn root(&self) -> &Self::Root;
}

impl<T: EventRoot> EventView for T {
    type Root = Self;
    type Parent = Self;

    fn from_root(event: Self::Root) -> Option<Self> {
        Some(event)
    }

    fn root(&self) -> &Self::Root {
        self
    }
}

pub type BoxedEvent = Arc<dyn EventRoot>;
