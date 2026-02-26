// ─── __plugin_metadata! ──────────────────────────────────────────────────────
//
// Internal helper macro. Called exclusively by `define_plugin!`.

#[macro_export]
macro_rules! __plugin_metadata {
    // Entry: receives provides list, doc comment, and raw metadata tokens
    (@parse [$($provide:ty),*] [$($doc:expr)?] $($meta:tt)*) => {
        $crate::__plugin_metadata!(
            @pm [$($provide),*] [$($doc)?] [] [] [] []
            :: $($meta)*
            ;;
        )
    };

    // TT-muncher for metadata tokens
    (@pm $p:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt
        :: , $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc $ver $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // version: "..."
    (@pm $p:tt $doc:tt [$($old:expr)?] $dsc:tt $mf:tt $pty:tt
        :: version : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc [$v] $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // desc: "..."
    (@pm $p:tt $doc:tt $ver:tt [$($old:expr)?] $mf:tt $pty:tt
        :: desc : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc $ver [$v] $mf $pty :: $($rest)* ;;
        )
    };

    // full_desc: "..."
    (@pm $p:tt $doc:tt $ver:tt $dsc:tt [$($old:expr)?] $pty:tt
        :: full_desc : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc $ver $dsc [$v] $pty :: $($rest)* ;;
        )
    };

    // plugin_type: <ident>
    (@pm $p:tt $doc:tt $ver:tt $dsc:tt $mf:tt [$($old:ident)?]
        :: plugin_type : $pt:ident $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc $ver $dsc $mf [$pt] :: $($rest)* ;;
        )
    };

    // ── end of metadata: skip unknown ident:value pairs ─────────────────────
    (@pm $p:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt
        :: $ident:ident : $value:tt $(, $(..)?)? $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $p $doc $ver $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // end of metadata → emit directly via @emit
    (@pm [$($provide:ty),*] [$($doc:expr)?] [$($ver:expr)?] [$($dsc:expr)?] [$($mf:expr)?] [$($pty:ident)?]
        :: ;;
    ) => {
        $crate::__plugin_metadata!(
            @emit [$($provide),*] [$($doc)?] [$($ver)?] [$($dsc)?] [$($mf)?] [$($pty)?]
        )
    };

    // @get_ver — resolve version: explicit value or env default
    (@get_ver []) => { ::std::env!("CARGO_PKG_VERSION") };
    (@get_ver [$ver:expr]) => { $ver };

    // @get_dsc — resolve desc: explicit value or env default
    (@get_dsc []) => { ::std::env!("CARGO_PKG_DESCRIPTION") };
    (@get_dsc [$dsc:expr]) => { $dsc };

    // @get_fd — resolve full_desc: explicit > doc > None
    (@get_fd [$fd:expr] [$_doc:tt]) => { ::std::option::Option::Some($fd) };
    (@get_fd [] [$doc:expr]) => { ::std::option::Option::Some($doc) };
    (@get_fd [] []) => { ::std::option::Option::None };

    // @get_type — resolve plugin_type: explicit > inferred from provides (returns full PluginType::xxx)
    (@get_type [] []) => { $crate::plugin::PluginType::Runtime };
    (@get_type [$_head:ty $(, $_:ty)*] []) => { $crate::plugin::PluginType::Service };
    (@get_type $_p:tt [service]) => { $crate::plugin::PluginType::Service };
    (@get_type $_p:tt [runtime]) => { $crate::plugin::PluginType::Runtime };

    // @emit — unified emission rule
    (@emit $p:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt) => {
        $crate::plugin::PluginMetadata {
            version: $crate::__plugin_metadata!(@get_ver $ver),
            plugin_type: $crate::__plugin_metadata!(@get_type $p $pty),
            desc: $crate::__plugin_metadata!(@get_dsc $dsc),
            full_desc: $crate::__plugin_metadata!(@get_fd $mf $doc),
        }
    };
}

// ─── define_plugin! ──────────────────────────────────────────────────────────

/// Creates a [`PluginDescriptor`] — the static, `Copy` handle to a plugin.
///
/// # Syntax
///
/// ```rust,ignore
/// use alloy::prelude::*;
///
/// // Fields may appear in **any order** after `name:`.
/// async fn my_on_load(ctx: PluginLoadContext) -> anyhow::Result<()> {
///     let cfg = ctx.get_config::<MyCfg>()?;
///     info!("my_plugin ready");
///     Ok(())
/// }
/// async fn my_on_unload() {
///     info!("unloaded");
/// }
///
/// pub static MY_PLUGIN: PluginDescriptor = define_plugin! {
///     name: "my_plugin",
///     depends_on: [StorageService],
///     provides:   [MyService],
///     handlers:   [on_message().handler(log_handler)],
///     on_load:    my_on_load,
///     on_unload:  my_on_unload,
///     metadata: {
///         version:     "2.0.0",
///         desc:        "Short.",
///         plugin_type: service,
///     },
/// };
/// ```
///
/// ## Field reference
///
/// | Field | Required | Description |
/// |-------|----------|-------------|
/// | `name` | ✓ | Must be **first**. Plugin display name. |
/// | `handlers` | — | Tower handler list `[expr, …]` |
/// | `provides` | — | `[ServiceType, …]` |
/// | `depends_on` | — | `[ServiceType, …]` |
/// | `on_load` | — | `async fn(ctx: PluginLoadContext)` function path |
/// | `on_unload` | — | `async fn()` function path |
/// | `metadata` | — | `{ version, desc, full_desc, plugin_type }` |
///
/// ## `metadata` block
///
/// All fields optional. Doc comment before `name:` becomes `full_desc` if not
/// set explicitly in metadata.
///
/// | Key | Type | Default |
/// |-----|------|---------|
/// | `version` | string literal | `CARGO_PKG_VERSION` |
/// | `desc` | string literal | `CARGO_PKG_DESCRIPTION` |
/// | `full_desc` | string literal | doc comment, or `None` |
/// | `plugin_type` | `service` \| `runtime` | auto-inferred from `provides` |
///
/// [`PluginDescriptor`]: crate::plugin::PluginDescriptor
#[macro_export]
macro_rules! define_plugin {
    // Entry points
    ($(#[doc = $doc:literal])+ name: $name:literal, $($tail:tt)+) => {
        $crate::define_plugin!(
            @acc [$name] [] [] [] [] [] [::std::concat!($($doc, " "),*)]
            $($tail)+
        )
    };

    // ── Entry: no doc + more content ─────────────────────────────────────────
    (name: $name:literal, $($tail:tt)+) => {
        $crate::define_plugin!(
            @acc [$name] [] [] [] [] [] []
            $($tail)+
        )
    };

    // Accumulator for non-metadata fields
    (@acc $n:tt $p:tt $d:tt $h:tt $lo:tt $un:tt $doc:tt
        , $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc $n $p $d $h $lo $un $doc
            $($rest)*
        )
    };

    // Consume provides
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        provides: [$($np:ty),* $(,)?] $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p,)* $($np),*] [$($d),*] [$($h),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // Consume depends_on
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        depends_on: [$($nd:ty),* $(,)?] $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d,)* $($nd),*] [$($h),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // Consume handlers
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        handlers: [$($nh:expr),* $(,)?] $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d),*] [$($h,)* $($nh),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // Consume on_load: a function `async fn(ctx: PluginLoadContext)` — followed by more fields
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [] [$($un:expr)?] $doc:tt
        on_load: $lo:path , $($rest:tt)+
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d),*] [$($h),*] [$lo] [$($un)?] $doc
            $($rest)+
        )
    };
    // Consume on_load: last field (trailing comma optional)
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [] [$($un:expr)?] [$($doc:expr)?]
        on_load: $lo:path $(,)?
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d),*] [$($h),*] [$lo] [$($un)?] [$($doc)?]
        )
    };

    // Consume on_unload: a function `async fn()` — followed by more fields
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [] $doc:tt
        on_unload: $un:path , $($rest:tt)+
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d),*] [$($h),*] [$($lo)?] [$un] $doc
            $($rest)+
        )
    };
    // Consume on_unload: last field (trailing comma optional)
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [] [$($doc:expr)?]
        on_unload: $un:path $(,)?
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($p),*] [$($d),*] [$($h),*] [$($lo)?] [$un] [$($doc)?]
        )
    };

    // Consume metadata block
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] [$($doc:expr)?]
        metadata: { $($meta:tt)* } $(,)?
    ) => {
        $crate::define_plugin!(
            @terminal [$n] [$($p),*] [$($d),*] [$($h),*] [$($lo)?] [$($un)?]
                [$($doc)?] $($meta)*
        )
    };

    // No metadata block
    (
        @acc [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] [$($doc:expr)?]
    ) => {
        $crate::define_plugin!(
            @terminal [$n] [$($p),*] [$($d),*] [$($h),*] [$($lo)?] [$($un)?]
                [$($doc)?]
        )
    };

    // @terminal — emit the PluginDescriptor
    (
        @terminal [$n:literal] [$($p:ty),*] [$($d:ty),*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?]
            [$($doc:expr)?] $($meta:tt)*
    ) => {{
        const __ALLOY_PROVIDES_IDS:   &[&str] = &[$(<$p>::ID),*];
        const __ALLOY_DEPENDS_ON_IDS: &[&str] = &[$(<$d>::ID),*];

        // Build metadata via __plugin_metadata!
        // __plugin_metadata! handles all parsing: version, desc, full_desc, plugin_type
        const __ALLOY_META: $crate::plugin::PluginMetadata =
            $crate::__plugin_metadata!(
                @parse [$($p),*] [$($doc)?] $($meta)*
            );

        fn __alloy_plugin_create() -> $crate::plugin::Plugin {
            $crate::plugin::Plugin::__new(
                $n,
                __ALLOY_DEPENDS_ON_IDS.to_vec(),
                vec![$( $crate::plugin::__BoxCloneSyncService::new($h) ),*],
                vec![$(
                    $crate::plugin::ServiceEntry {
                        id:      <$p>::ID,
                        type_id: ::std::any::TypeId::of::<$p>(),
                        factory: ::std::sync::Arc::new(|ctx: ::std::sync::Arc<$crate::plugin::PluginLoadContext>| {
                            ::std::boxed::Box::pin(async move {
                                ::std::sync::Arc::new(
                                    <$p as $crate::plugin::PluginService>::init(ctx).await
                                ) as ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>
                            })
                        }),
                    }
                ),*],
                {
                    #[allow(unused_mut)]
                    let mut __f: ::std::option::Option<$crate::plugin::OnLoadFn> = None;
                    $(
                        __f = Some(::std::sync::Arc::new(
                            |ctx: $crate::plugin::PluginLoadContext| {
                                ::std::boxed::Box::pin($lo(ctx))
                            },
                        ));
                    )?
                    __f
                },
                {
                    #[allow(unused_mut)]
                    let mut __f: ::std::option::Option<$crate::plugin::OnUnloadFn> = None;
                    $(
                        __f = Some(::std::sync::Arc::new(
                            || ::std::boxed::Box::pin($un()),
                        ));
                    )?
                    __f
                },
                __ALLOY_META,
            )
        }

        $crate::plugin::PluginDescriptor {
            api_version: $crate::plugin::ALLOY_PLUGIN_API_VERSION,
            name:        $n,
            provides:    __ALLOY_PROVIDES_IDS,
            depends_on:  __ALLOY_DEPENDS_ON_IDS,
            create:      __alloy_plugin_create,
            metadata:    __ALLOY_META,
        }
    }};
}
