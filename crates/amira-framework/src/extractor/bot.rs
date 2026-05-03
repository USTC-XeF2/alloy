use std::sync::Arc;

use crate::context::{BoxedBot, HandlerContext};
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use amira_core::error::ApiResult;
use amira_core::{ApiExecutor, ApiPayload, Bot};

/// Context wrapper that provides access to the bot instance.
///
/// This is the primary way handlers receive and use the bot. Use `Deref` to access
/// the bot interface directly.
#[derive(Debug, Clone)]
pub struct BotClient<T: Bot + ApiExecutor<Bot = T>>(Arc<T>);

impl<T> BotClient<T>
where
    T: Bot + ApiExecutor<Bot = T>,
{
    pub fn id(&self) -> &str {
        self.0.id()
    }
}

impl<T> ApiExecutor for BotClient<T>
where
    T: Bot + ApiExecutor<Bot = T>,
{
    type Bot = T;

    async fn execute<U: ApiPayload<Bot = Self::Bot>>(&self, payload: U) -> ApiResult<U::Response> {
        self.0.execute(payload).await
    }
}

/// Implementation for extracting `BotClient<T>` where `T: Bot`.
///
/// This enables handlers to inject a concrete bot type and access protocol-specific APIs:
impl<T> FromContext for BotClient<T>
where
    T: Bot + ApiExecutor<Bot = T>,
{
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        ctx.bot()
            .clone()
            .downcast_arc::<T>()
            .map(BotClient)
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
/// use amira_core::BoxedBot;
///
/// async fn my_handler(bot: BoxedBot, event: EventContext<MessageEvent>) {
///     // Use the bot to send a message back
///     bot.send(event.as_ref(), "Hello!").await.ok();
/// }
/// ```
impl FromContext for BoxedBot {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(ctx.bot().clone())
    }
}
