//! Extractor system for the Alloy framework.
//!
//! This module provides the [`FromContext`] trait, which defines how types
//! can be extracted from an [`AlloyContext`] for use as handler parameters.

use crate::error::ExtractError;
use alloy_core::foundation::context::AlloyContext;
use alloy_core::foundation::event::{EventContext, FromEvent};
use alloy_core::integration::bot::BoxedBot;

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
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::{AlloyContext, FromContext, ExtractError};
/// use std::sync::Arc;
///
/// struct MyExtractor {
///     data: String,
/// }
///
/// impl FromContext for MyExtractor {
///     fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
///         // Custom extraction logic here
///         Ok(MyExtractor { data: "extracted".into() })
///     }
/// }
/// ```
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
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError>;
}

/// Blanket implementation for extracting the event as a clone of [`BoxedEvent`].
///
/// This is useful when a handler needs to work with any event type
/// without knowing the concrete type at compile time.
impl FromContext for alloy_core::foundation::event::BoxedEvent {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        Ok(ctx.event().clone())
    }
}

/// Implementation for `Option<T>` where `T: FromContext`.
///
/// This allows handlers to have optional parameters that may or may not
/// be extractable from the context.
impl<T: FromContext> FromContext for Option<T> {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        Ok(T::from_context(ctx).ok())
    }
}

/// Implementation for extracting `EventContext<T>` where `T: FromEvent`.
///
/// This is the Clap-like pattern where handlers can request events at any
/// level of the hierarchy:
///
/// ```rust,ignore
/// use alloy_core::EventContext;
///
/// // Extract a specific event type
/// async fn on_poke(event: EventContext<PokeNotifyEvent>) {
///     println!("Target: {}", event.target_id);
/// }
///
/// // Extract an intermediate event type
/// async fn on_notice(event: EventContext<NoticeEvent>) {
///     match &event.inner {
///         NoticeType::Poke(p) => println!("Poke: {}", p.target_id),
///         _ => {}
///     }
/// }
/// ```
impl<T: FromEvent + Clone + alloy_core::Event> FromContext for EventContext<T> {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        ctx.event()
            .extract::<T>()
            .ok_or_else(|| ExtractError::EventTypeMismatch {
                expected: std::any::type_name::<T>(),
                got: ctx.event().event_name(),
            })
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
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        Ok(ctx.bot_arc())
    }
}

/// Implementation for extracting a specific Bot type from context.
///
/// This allows handlers to inject a concrete bot type and access protocol-specific APIs:
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use alloy_adapter_onebot::OneBotBot;
///
/// async fn my_handler(bot: Arc<OneBotBot>, event: EventContext<MessageEvent>) {
///     // Use protocol-specific APIs
///     bot.send_private_msg(12345, "Hello!", false).await.ok();
/// }
/// ```
impl<T: alloy_core::integration::bot::Bot + 'static> FromContext for std::sync::Arc<T> {
    fn from_context(ctx: &AlloyContext) -> Result<Self, ExtractError> {
        use alloy_core::integration::bot::downcast_bot;

        // Get the BoxedBot
        let boxed_bot = ctx.bot_arc();

        // Try to downcast to the concrete type
        downcast_bot::<T>(boxed_bot).ok_or_else(|| ExtractError::BotTypeMismatch {
            expected: std::any::type_name::<T>(),
        })
    }
}
