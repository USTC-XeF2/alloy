use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;
use alloy_core::Bot as BotTrait;

/// Context wrapper that provides access to the bot instance.
///
/// This is the primary way handlers receive and use the bot. Use `Deref` to access
/// the bot interface directly.
pub struct Bot<T: BotTrait>(pub Arc<T>);

impl<T: BotTrait> Bot<T> {
    /// Creates a new Bot wrapper with the given bot instance.
    pub(crate) fn new(bot: Arc<T>) -> Self {
        Self(bot)
    }
}

impl<T: BotTrait> std::ops::Deref for Bot<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: BotTrait> AsRef<dyn BotTrait> for Bot<T> {
    fn as_ref(&self) -> &dyn BotTrait {
        self.0.as_ref()
    }
}

impl<T: BotTrait + std::fmt::Debug> std::fmt::Debug for Bot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot").field("bot", &self.0).finish()
    }
}

/// Implementation for extracting `Bot<T>` where `T: Bot`.
///
/// This enables handlers to inject a concrete bot type and access protocol-specific APIs:
#[async_trait]
impl<T: BotTrait> FromContext for Bot<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        // Get the BoxedBot
        let boxed_bot = ctx.bot_arc();

        // Try to downcast to the concrete type
        let any_arc = boxed_bot.as_any();
        Arc::downcast::<T>(any_arc)
            .map(Bot::new)
            .map_err(|_| ExtractError::BotTypeMismatch {
                expected: std::any::type_name::<T>(),
            })
    }
}
