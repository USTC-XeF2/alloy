use crate::context::AlloyContext;
use crate::error::ExtractResult;
use alloy_core::{BoxedBot, BoxedEvent};

/// A trait for types that can be extracted from an [`AlloyContext`].
///
/// This is the core abstraction that enables the Alloy framework's parameter
/// injection system. Types implementing this trait can be used directly as
/// handler function parameters.
///
/// # Error Handling
///
/// The extraction can fail (returning `Err`) if the required data is not
/// available in the context. In this case, the handler will be skipped.
pub trait FromContext: Sized {
    /// Attempts to extract this type from the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context to extract from.
    ///
    /// # Returns
    ///
    /// `Ok(Self)` if extraction succeeds, `Err(ExtractError)` otherwise.
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self>;
}

/// Blanket implementation for extracting the event as a clone of [`BoxedEvent`].
///
/// This is useful when a handler needs to work with any event type
/// without knowing the concrete type at compile time.
impl FromContext for BoxedEvent {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        Ok(ctx.event().clone())
    }
}

/// Implementation for `Option<T>` where `T: FromContext`.
///
/// This allows handlers to have optional parameters that may or may not
/// be extractable from the context.
impl<T: FromContext> FromContext for Option<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        Ok(T::from_context(ctx).ok())
    }
}

/// Implementation for extracting the Bot from context.
///
/// This allows handlers to inject the bot and use it to send messages:
///
/// ```rust,ignore
/// use alloy_core::BoxedBot;
///
/// async fn my_handler(bot: BoxedBot, event: EventContext<MessageEvent>) {
///     // Use the bot to send a message back
///     bot.send(event.as_event(), "Hello!").await.ok();
/// }
/// ```
impl FromContext for BoxedBot {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        Ok(ctx.bot_arc())
    }
}
