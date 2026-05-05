use super::context::PluginLoadContext;
use super::descriptor::{BoxedHandlerService, DependsOnEntry, PluginDescriptor, ServiceEntry};

/// A live plugin instance bundling handlers and a reference to its descriptor.
///
/// Create via the [`define_plugin!`] macro.
///
/// # Concurrency
///
/// `Plugin` is `Send + Sync`.  Use interior mutability (e.g. `Arc<Mutex<T>>`)
/// for state that changes across events.
#[derive(Debug)]
pub struct Plugin {
    descriptor: &'static PluginDescriptor,
    handlers: Vec<BoxedHandlerService>,
}

impl Plugin {
    /// Creates the live plugin from a descriptor.
    pub(crate) fn new(descriptor: &'static PluginDescriptor) -> Self {
        Plugin {
            handlers: (descriptor.create_handlers)(),
            descriptor,
        }
    }

    /// Returns the plugin's display name.
    pub fn name(&self) -> &'static str {
        self.descriptor.name
    }

    /// Service IDs this plugin registers into the global registry.
    pub fn provides(&self) -> Vec<&'static str> {
        self.descriptor.provides.iter().map(|e| e.id).collect()
    }

    /// All dependencies declared in this plugin's `depends_on: [...]` list.
    pub fn depends_on(&self) -> &[DependsOnEntry] {
        self.descriptor.depends_on
    }

    /// Returns a slice of this plugin's handlers for the [`PluginManager`] to drive.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    pub(crate) fn handlers(&self) -> &[BoxedHandlerService] {
        &self.handlers
    }

    /// Service factory entries declared by this plugin.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    pub(crate) fn service_entries(&self) -> &[ServiceEntry] {
        self.descriptor.provides
    }

    /// Called once at startup, **before** services declared in `provides` are
    /// registered.
    ///
    /// Returns `Ok(())` when the plugin loaded successfully.  Returning `Err`
    /// causes [`PluginManager`] to mark the plugin as
    /// [`PluginLoadState::Failed`] and skip service registration entirely.
    ///
    /// [`PluginManager`]: crate::manager::PluginManager
    /// [`PluginLoadState::Failed`]: crate::manager::PluginLoadState::Failed
    pub(crate) async fn on_load(
        &self,
        ctx: PluginLoadContext,
    ) -> Result<(), Box<dyn std::fmt::Display + Send>> {
        if let Some(f) = &self.descriptor.on_load {
            f(ctx).await
        } else {
            Ok(())
        }
    }

    /// Called once at shutdown.
    pub(crate) async fn on_unload(&self) {
        if let Some(f) = &self.descriptor.on_unload {
            f().await;
        }
    }
}
