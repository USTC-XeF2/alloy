//! [`PluginConfig<T>`] — handler extractor for plugin configuration.

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::context::AlloyContext;
use crate::error::ExtractResult;
use crate::extractor::FromContext;

/// Extractor that provides a handler with its plugin's typed configuration.
///
/// The runtime automatically injects the plugin's raw JSON section from
/// `alloy.yaml → plugins.<plugin_name>` into every [`AlloyContext`] before
/// the handler chain runs.  `PluginConfig<T>` deserialises that JSON into `T`.
///
/// If the YAML section is absent or empty, `T::default()` is used (requires
/// `T: Default`).  If deserialisation fails the handler is skipped with
/// [`ExtractError::MissingState`].
///
/// # Example
///
/// ```rust,ignore
/// #[derive(serde::Deserialize, Default)]
/// struct BotConfig {
///     prefix: String,
///     max_length: usize,
/// }
///
/// async fn handler(
///     event: EventContext<MessageEvent>,
///     cfg:   PluginConfig<BotConfig>,
/// ) -> anyhow::Result<String> {
///     Ok(format!("{} {}", cfg.prefix, event.get_plain_text()))
/// }
/// ```
///
/// YAML (`alloy.yaml`):
/// ```yaml
/// plugins:
///   my_plugin:
///     prefix: "[Bot]"
///     max_length: 200
/// ```
pub struct PluginConfig<T>(pub Arc<T>);

impl<T> std::ops::Deref for PluginConfig<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: DeserializeOwned + Default + Send + Sync + 'static> FromContext for PluginConfig<T> {
    fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        // The runtime injects the plugin's config JSON section directly into context.
        // An absent section yields Value::Null which serde_json treats as empty for structs-with-defaults.
        let json = ctx.config(); // &Arc<serde_json::Value>
        let t: T = serde_json::from_value((**json).clone()).unwrap_or_default();
        Ok(PluginConfig(Arc::new(t)))
    }
}
