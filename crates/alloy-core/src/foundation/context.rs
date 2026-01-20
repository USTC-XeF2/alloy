//! Context and extractor system for the Alloy framework.
//!
//! This module provides [`AlloyContext`], the central context object that wraps
//! events and manages propagation state during handler execution.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::foundation::event::BoxedEvent;
use crate::integration::bot::BoxedBot;

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
///     if let Some(bot) = ctx.bot() {
///         bot.send(ctx.event().inner().as_ref(), "Hello!").await.ok();
///     }
/// }
/// ```
pub struct AlloyContext {
    /// The wrapped event being processed.
    event: BoxedEvent,
    /// The bot associated with this event (if any).
    bot: Option<BoxedBot>,
    /// Flag indicating whether the event should continue propagating to other handlers.
    is_propagating: AtomicBool,
}

impl AlloyContext {
    /// Creates a new context wrapping the given event.
    ///
    /// The context is initialized with propagation enabled, meaning subsequent
    /// handlers will receive the event unless [`stop_propagation`](Self::stop_propagation)
    /// is called.
    ///
    /// # Arguments
    ///
    /// * `event` - The boxed event to wrap.
    ///
    /// # Returns
    ///
    /// A new `AlloyContext` instance.
    pub fn new(event: BoxedEvent) -> Self {
        Self {
            event,
            bot: None,
            is_propagating: AtomicBool::new(true),
        }
    }

    /// Creates a new context with an associated bot.
    pub fn with_bot(event: BoxedEvent, bot: BoxedBot) -> Self {
        Self {
            event,
            bot: Some(bot),
            is_propagating: AtomicBool::new(true),
        }
    }

    /// Sets the bot for this context.
    pub fn set_bot(&mut self, bot: BoxedBot) {
        self.bot = Some(bot);
    }

    /// Returns a reference to the underlying boxed event.
    ///
    /// # Returns
    ///
    /// A reference to the [`BoxedEvent`].
    pub fn event(&self) -> &BoxedEvent {
        &self.event
    }

    /// Returns a reference to the bot, if available.
    pub fn bot(&self) -> Option<&BoxedBot> {
        self.bot.as_ref()
    }

    /// Returns a clone of the bot Arc, if available.
    pub fn bot_arc(&self) -> Option<BoxedBot> {
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
}

impl std::fmt::Debug for AlloyContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlloyContext")
            .field("event", &self.event)
            .field("has_bot", &self.bot.is_some())
            .field("is_propagating", &self.is_propagating())
            .finish()
    }
}
