//! Procedural macros for the Alloy bot framework.

mod capability;
mod event;
mod plugin;

use proc_macro::TokenStream;
use syn::parse_macro_input;

/// EventView root macro.
#[proc_macro_attribute]
pub fn event_root(attr: TokenStream, item: TokenStream) -> TokenStream {
    match event::expand_event_root(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// EventView data macro.
///
/// Apply to enums marked as event data containers. This will generate:
/// - aggregate helper methods (write_id/event_type/scene/plain_text/rich_text)
/// - view structs declared by variant-level #[event_view(...)]
/// - EventView + Deref impls for generated view structs
#[proc_macro_attribute]
pub fn event_data(attr: TokenStream, item: TokenStream) -> TokenStream {
    match event::expand_event_data(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Registers an async function as a transport capability implementation.
///
/// # Usage
///
/// Apply this attribute macro to an async function matching one of the five capability types:
///
/// ```rust,ignore
/// #[alloy_macros::register_capability(ws_client)]
/// pub async fn ws_connect(
///     config: WsClientConfig,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<()> { ... }
///
/// #[alloy_macros::register_capability(ws_server)]
/// pub async fn ws_listen(
///     config: WsServerConfig,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<()> { ... }
///
/// #[alloy_macros::register_capability(http_client)]
/// pub async fn http_start_client(
///     config: HttpClientConfig,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<()> { ... }
///
/// #[alloy_macros::register_capability(http_server)]
/// pub async fn http_listen(
///     config: HttpServerConfig,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<()> { ... }
///
/// #[alloy_macros::register_capability(sse_client)]
/// pub async fn sse_start_client(
///     config: SseClientConfig,
///     handler: Arc<dyn ConnectionHandler>,
/// ) -> TransportResult<()> { ... }
/// ```
///
/// The macro leaves the decorated function unchanged and emits a `#[linkme::distributed_slice]`
/// static that wires the function into the capability registry in `alloy-core`.
///
/// The runtime calls [`TransportContext::collect_all()`] once at startup to
/// gather all registered capabilities.
///
/// The attribute argument must be one of: `ws_client`, `ws_server`, `http_client`, `http_server`, `sse_client`.
#[proc_macro_attribute]
pub fn register_capability(attr: TokenStream, item: TokenStream) -> TokenStream {
    capability::register_capability(attr, item)
}

/// Creates a `pub static` [`PluginDescriptor`] — the static handle to a plugin.
///
/// The name of the generated static is the **uppercase form** of `name` with `_PLUGIN` suffix.
///
/// # Syntax
///
/// ```rust,ignore
/// define_plugin! {
///     /// Optional doc comment — attached to the static AND used as `full_desc`.
///     name: "my_plugin",                       // required, must be first (after docs)
///
///     // Services this plugin registers at load time (Trait → impl mapping)
///     provides: {
///         MyService: MyServiceImpl,
///     },
///
///     // Service traits that must be loaded before this plugin
///     depends_on: [MyOtherService],
///
///     // Tower handler services (built with on_message(), on_command(), …)
///     handlers: [
///         on_message().handler(my_handler),
///         on_command::<MyCmd>("cmd").handler(cmd_handler),
///     ],
///
///     on_load:   my_on_load_fn,    // async fn(PluginLoadContext) -> Result<()>
///     on_unload: my_on_unload_fn,  // async fn()
///
///     metadata: {
///         version:     "2.0.0",          // default: CARGO_PKG_VERSION
///         desc:        "Short summary.",  // default: CARGO_PKG_DESCRIPTION
///         full_desc:   "Longer text.",    // overrides doc comment
///         plugin_type: service,           // `service` or `runtime`; auto-inferred
///     },
/// }
/// ```
///
/// ## Field reference
///
/// | Field | Required | Description |
/// |-------|----------|-------------|
/// | `name` | ✓ | Must be **first** (after optional docs). Display name and config-section key. |
/// | `provides` | — | `{ Trait: ImplType, … }` — services injected into the registry |
/// | `depends_on` | — | `[Trait, …]` — traits that must exist before loading |
/// | `handlers` | — | `[expr, …]` — Tower handler services |
/// | `on_load` | — | `async fn(PluginLoadContext) -> Result<()>` |
/// | `on_unload` | — | `async fn()` |
/// | `metadata` | — | `{ version, desc, full_desc, plugin_type }` |
///
/// [`PluginDescriptor`]: alloy_framework::plugin::PluginDescriptor
#[proc_macro]
pub fn define_plugin(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as plugin::DefinePluginInput);
    plugin::expand(parsed).into()
}

/// Automatically generates `ServiceMeta` implementation for a service trait.
///
/// Apply this attribute macro to a trait definition with a service ID string.
/// It generates an `impl ServiceMeta for dyn YourTrait { const ID: &'static str = "…"; }`.
///
/// # Usage
///
/// ```rust,ignore
/// use alloy_macros::service_meta;
///
/// #[service_meta("storage")]
/// pub trait StorageService: Send + Sync {
///     fn cache_dir(&self) -> PathBuf;
///     fn data_dir(&self) -> PathBuf;
/// }
/// ```
///
/// This expands to:
///
/// ```rust,ignore
/// pub trait StorageService: Send + Sync {
///     fn cache_dir(&self) -> PathBuf;
///     fn data_dir(&self) -> PathBuf;
/// }
///
/// impl ServiceMeta for dyn StorageService {
///     const ID: &'static str = "storage";
/// }
/// ```
#[proc_macro_attribute]
pub fn service_meta(attr: TokenStream, item: TokenStream) -> TokenStream {
    plugin::expand_service_meta(attr.into(), item.into()).into()
}
