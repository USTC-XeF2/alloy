//! Runtime handles for querying and sharing runtime-owned resources.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use alloy_core::{Bot, BoxedBot, BridgeRuntime};
use alloy_framework::{
    context::AlloyContext,
    error::{ExtractError, ExtractResult},
    extractor::FromContext,
};

/// Handle for querying bot instances managed by all registered adapters.
///
/// This handle is cloneable and can be injected into handlers via `FromContext`.
#[derive(Clone)]
pub struct BotHandle(pub(crate) Arc<Mutex<HashMap<String, Arc<dyn BridgeRuntime>>>>);

impl BotHandle {
    /// Gets a boxed bot instance by its ID across all adapters.
    pub fn get_boxed_bot(&self, bot_id: &str) -> Option<BoxedBot> {
        for bridge in self.0.lock().values() {
            if let Some(bot) = bridge.bots().into_iter().find(|b| b.id() == bot_id) {
                return Some(bot);
            }
        }
        None
    }

    /// Gets a bot instance by its ID and downcasts it to the specified type.
    pub fn get_bot<T: Bot>(&self, bot_id: &str) -> Option<Arc<T>> {
        self.get_boxed_bot(bot_id)?.downcast_arc::<T>().ok()
    }
}

impl FromContext for BotHandle {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.plugin()
            .state()
            .get::<Self>()
            .ok_or(ExtractError::StateNotFound("BotHandle"))
    }
}
