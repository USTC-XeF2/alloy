//! Plugin descriptor — the static, `Copy` handle to a plugin.

use super::core::{DependsOnEntry, Plugin, PluginMetadata};

// ─── API versioning ─────────────────────────────────────────────────────────────────────────────

/// Current Amira plugin API version (1.0).
pub const AMIRA_PLUGIN_API_VERSION: u32 = 0x0001_0000;

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
    /// Plugin API version this descriptor was compiled against.
    pub api_version: u32,

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
    /// Returns `true` if this descriptor's API version is compatible with the
    /// running framework.
    ///
    /// The major part must match exactly; the descriptor's minor part must be
    /// ≤ the host's minor part.
    pub fn is_compatible(&self) -> bool {
        let host_major = AMIRA_PLUGIN_API_VERSION >> 16;
        let desc_major = self.api_version >> 16;
        let desc_minor = self.api_version & 0xFFFF;
        let host_minor = AMIRA_PLUGIN_API_VERSION & 0xFFFF;
        desc_major == host_major && desc_minor <= host_minor
    }

    /// Creates the live plugin from the factory function.
    ///
    /// Prefer [`AmiraRuntime::register_plugin`] which also handles the
    /// compatibility check, config initialisation, and registration.
    #[inline]
    pub fn instantiate(&self) -> Plugin {
        (self.create)()
    }
}
