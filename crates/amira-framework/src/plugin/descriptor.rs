//! Plugin descriptor — the static, `Copy` handle to a plugin.

use super::core::{DependsOnEntry, Plugin, PluginMetadata};

// ─── PluginDescriptor ─────────────────────────────────────────────────────────

/// A static, `Copy` descriptor that identifies and instantiates a plugin.
///
/// # Creating descriptors
///
/// Use the [`plugin!`] macro — it produces a `PluginDescriptor` that can be
/// stored in a `static` item, passed as a function argument, or used inline.
///
/// # Memory layout
///
/// `PluginDescriptor` is `#[repr(C)]`.  Fields **must not be reordered**.
#[repr(C)]
#[derive(Debug)]
pub struct PluginDescriptor {
    /// Human-readable plugin name (used in logs and as config lookup key).
    pub name: &'static str,

    /// Service IDs registered into the global service map during load.
    pub provides: &'static [&'static str],

    /// All dependencies declared in `depends_on: [...]`, with `required` flag indicating
    /// whether each is mandatory (`true` for marked with `!`) or optional (`false`).
    pub depends_on: &'static [DependsOnEntry],

    /// Factory function that creates the live [`Plugin`] instance.
    pub create: fn() -> Plugin,

    /// Static metadata snapshot for this plugin.
    pub metadata: PluginMetadata,
}

impl PluginDescriptor {
    /// Creates the live plugin from the factory function.
    ///
    /// Prefer [`AmiraRuntime::register_plugin`] which also handles the
    /// compatibility check, config initialisation, and registration.
    #[inline]
    pub fn instantiate(&self) -> Plugin {
        (self.create)()
    }
}
