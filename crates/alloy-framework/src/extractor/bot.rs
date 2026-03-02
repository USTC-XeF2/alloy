use std::sync::Arc;

use derive_more::{AsRef, Deref};

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use alloy_core::{Bot as BotTrait, BoxedBot};

/// Context wrapper that provides access to the bot instance.
///
/// This is the primary way handlers receive and use the bot. Use `Deref` to access
/// the bot interface directly.
#[derive(Debug, Clone, Deref, AsRef)]
#[as_ref(dyn BotTrait)]
pub struct Bot<T: BotTrait>(Arc<T>);

/// Implementation for extracting `Bot<T>` where `T: Bot`.
///
/// This enables handlers to inject a concrete bot type and access protocol-specific APIs:
impl<T: BotTrait> FromContext for Bot<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.bot()
            .clone()
            .as_any()
            .downcast::<T>()
            .map(Bot)
            .map_err(|_| ExtractError::BotTypeMismatch {
                expected: std::any::type_name::<T>(),
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
///     bot.send(event.as_ref(), "Hello!").await.ok();
/// }
/// ```
impl FromContext for BoxedBot {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        Ok(ctx.bot().clone())
    }
}
