use std::sync::Arc;

use crate::context::PluginContext;

/// Load-time plugin context that extends [`PluginContext`] with command registration.
///
/// This context is only used by plugin lifecycle hooks such as `on_load`.
#[derive(Clone)]
pub struct PluginLoadContext {
    plugin: Arc<PluginContext>,
    #[cfg(feature = "command")]
    command: Arc<crate::context::CommandContext>,
}

impl PluginLoadContext {
    pub(crate) fn new(
        plugin: Arc<PluginContext>,
        #[cfg(feature = "command")] command: Arc<crate::context::CommandContext>,
    ) -> Self {
        Self {
            plugin,
            #[cfg(feature = "command")]
            command,
        }
    }

    /// Registers this plugin's help provider into the runtime-scoped command context.
    #[cfg(feature = "command")]
    pub fn register_commands<F, T>(&self, provider: F)
    where
        F: crate::handler::FromCtxFn<T, Response = crate::command::CommandMap>,
        T: Send + Sync + 'static,
    {
        self.command.help_provider.lock().insert(
            self.plugin.name().to_string(),
            Arc::new((provider, std::marker::PhantomData)),
        );
    }
}

impl std::ops::Deref for PluginLoadContext {
    type Target = PluginContext;

    fn deref(&self) -> &Self::Target {
        &self.plugin
    }
}
