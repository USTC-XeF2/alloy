//! Context and extractor system for the Alloy framework.
//!
//! This module provides [`AlloyContext`], the central context object that wraps
//! events and manages propagation state during handler execution.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use alloy_core::{BoxedBot, BoxedEvent};

/// The context object passed to handlers during event processing.
///
/// `AlloyContext` wraps a [`BoxedEvent`] and provides additional state management
/// for controlling event propagation through the handler chain.
///
/// # Thread Safety
///
/// `AlloyContext` is designed to be shared across async tasks. The propagation
/// state is managed using atomic operations for thread-safe access.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::{AlloyContext, BoxedEvent};
///
/// async fn handle_event(ctx: &AlloyContext) {
///     // Access the underlying event
///     println!("Processing event: {:?}", ctx.event());
///     
///     // Stop further handlers from processing this event
///     ctx.stop_propagation();
///     
///     // Access the bot to send messages
///     ctx.bot().send(ctx.event().deref(), "Hello!").await.ok();
/// }
/// ```
pub struct AlloyContext {
    /// The wrapped event being processed.
    event: BoxedEvent,
    /// The bot associated with this event.
    bot: BoxedBot,
    /// Flag indicating whether the event should continue propagating to other handlers.
    is_propagating: AtomicBool,
    /// Type-keyed state storage for passing data between matchers and handlers.
    state: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl AlloyContext {
    /// Creates a new context wrapping the given event and bot.
    ///
    /// The context is initialized with propagation enabled, meaning subsequent
    /// handlers will receive the event unless [`stop_propagation`](Self::stop_propagation)
    /// is called.
    ///
    /// # Arguments
    ///
    /// * `event` - The boxed event to wrap.
    /// * `bot` - The bot associated with this event.
    ///
    /// # Returns
    ///
    /// A new `AlloyContext` instance.
    pub fn new(event: BoxedEvent, bot: BoxedBot) -> Self {
        Self {
            event,
            bot,
            is_propagating: AtomicBool::new(true),
            state: RwLock::new(HashMap::new()),
        }
    }

    /// Returns a reference to the underlying boxed event.
    ///
    /// # Returns
    ///
    /// A reference to the [`BoxedEvent`].
    pub fn event(&self) -> &BoxedEvent {
        &self.event
    }

    /// Returns a reference to the bot.
    pub fn bot(&self) -> &BoxedBot {
        &self.bot
    }

    /// Returns a clone of the bot Arc.
    pub fn bot_arc(&self) -> BoxedBot {
        self.bot.clone()
    }

    /// Stops the event from propagating to further handlers.
    ///
    /// Once called, the dispatcher will not invoke any remaining handlers
    /// for this event.
    ///
    /// # Thread Safety
    ///
    /// This operation is atomic and safe to call from any thread.
    pub fn stop_propagation(&self) {
        self.is_propagating.store(false, Ordering::SeqCst);
    }

    /// Checks if the event is still propagating.
    ///
    /// # Returns
    ///
    /// `true` if the event should continue to subsequent handlers,
    /// `false` if propagation has been stopped.
    pub fn is_propagating(&self) -> bool {
        self.is_propagating.load(Ordering::SeqCst)
    }

    /// Stores a value in the context's type-keyed state.
    ///
    /// This allows matchers to store parsed data that handlers can later retrieve.
    /// Only one value per type can be stored; subsequent calls will overwrite.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // In a matcher's check function:
    /// ctx.set_state(MyParsedCommand { ... });
    ///
    /// // In a handler:
    /// if let Some(cmd) = ctx.get_state::<MyParsedCommand>() {
    ///     // Use the parsed command
    /// }
    /// ```
    pub fn set_state<T: Send + Sync + 'static>(&self, value: T) {
        let mut state = self.state.write().unwrap();
        state.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieves a value from the context's type-keyed state.
    ///
    /// Returns `None` if no value of the given type has been stored.
    pub fn get_state<T: Clone + 'static>(&self) -> Option<T> {
        let state = self.state.read().unwrap();
        state
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Checks if a value of the given type exists in state.
    pub fn has_state<T: 'static>(&self) -> bool {
        let state = self.state.read().unwrap();
        state.contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns a value from state.
    pub fn take_state<T: 'static>(&self) -> Option<T> {
        let mut state = self.state.write().unwrap();
        state
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }
}

impl std::fmt::Debug for AlloyContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_count = self.state.read().map(|s| s.len()).unwrap_or(0);
        f.debug_struct("AlloyContext")
            .field("event", &self.event)
            .field("is_propagating", &self.is_propagating())
            .field("state_entries", &state_count)
            .finish()
    }
}
