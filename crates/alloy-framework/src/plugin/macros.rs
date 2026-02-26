// ─── Internal helper: PluginMetadata builder ──────────────────────────────────
//
// Used exclusively by `define_plugin!`.  Not part of the public API.

/// Internal helper macro: builds a [`PluginMetadata`] from optional overrides.
///
/// # Internal calling convention
///
/// ```text
/// __plugin_metadata!(
///     @parse [yes | (empty)]   ← "has provides" flag
///            [$doc?]           ← captured doc literal
///            key: val, …       ← raw metadata tokens
/// )
/// ```
#[macro_export]
#[doc(hidden)]
macro_rules! __plugin_metadata {
    // Entry: receives has-provides flag, doc comment, and raw metadata tokens
    (@parse $hp:tt [$($doc:expr)?] $($meta:tt)*) => {
        $crate::__plugin_metadata!(
            @pm $hp [$($doc)?] [] [] [] []
            :: $($meta)*
            ;;
        )
    };

    // TT-muncher: skip leading comma
    (@pm $hp:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt
        :: , $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc $ver $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // version: "..."
    (@pm $hp:tt $doc:tt [$($old:expr)?] $dsc:tt $mf:tt $pty:tt
        :: version : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc [$v] $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // desc: "..."
    (@pm $hp:tt $doc:tt $ver:tt [$($old:expr)?] $mf:tt $pty:tt
        :: desc : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc $ver [$v] $mf $pty :: $($rest)* ;;
        )
    };

    // full_desc: "..."
    (@pm $hp:tt $doc:tt $ver:tt $dsc:tt [$($old:expr)?] $pty:tt
        :: full_desc : $v:literal $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc $ver $dsc [$v] $pty :: $($rest)* ;;
        )
    };

    // plugin_type: <ident>
    (@pm $hp:tt $doc:tt $ver:tt $dsc:tt $mf:tt [$($old:ident)?]
        :: plugin_type : $pt:ident $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc $ver $dsc $mf [$pt] :: $($rest)* ;;
        )
    };

    // Skip unknown ident:value pairs
    (@pm $hp:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt
        :: $ident:ident : $value:tt $($rest:tt)*
        ;;
    ) => {
        $crate::__plugin_metadata!(
            @pm $hp $doc $ver $dsc $mf $pty :: $($rest)* ;;
        )
    };

    // End of tokens → emit
    (@pm $hp:tt [$($doc:expr)?] [$($ver:expr)?] [$($dsc:expr)?] [$($mf:expr)?] [$($pty:ident)?]
        :: ;;
    ) => {
        $crate::__plugin_metadata!(
            @emit $hp [$($doc)?] [$($ver)?] [$($dsc)?] [$($mf)?] [$($pty)?]
        )
    };

    // @get_ver
    (@get_ver []) => { ::std::env!("CARGO_PKG_VERSION") };
    (@get_ver [$ver:expr]) => { $ver };

    // @get_dsc
    (@get_dsc []) => { ::std::env!("CARGO_PKG_DESCRIPTION") };
    (@get_dsc [$dsc:expr]) => { $dsc };

    // @get_fd: explicit > doc > None
    (@get_fd [$fd:expr] [$_doc:tt]) => { ::std::option::Option::Some($fd) };
    (@get_fd [] [$doc:expr]) => { ::std::option::Option::Some($doc) };
    (@get_fd [] []) => { ::std::option::Option::None };

    // @get_type: explicit override wins; else infer from has-provides flag
    (@get_type $_hp:tt [service]) => { $crate::plugin::PluginType::Service };
    (@get_type $_hp:tt [runtime]) => { $crate::plugin::PluginType::Runtime };
    (@get_type [] []) => { $crate::plugin::PluginType::Runtime };
    (@get_type [yes] []) => { $crate::plugin::PluginType::Service };

    // @emit — final struct
    (@emit $hp:tt $doc:tt $ver:tt $dsc:tt $mf:tt $pty:tt) => {
        $crate::plugin::PluginMetadata {
            version:     $crate::__plugin_metadata!(@get_ver $ver),
            plugin_type: $crate::__plugin_metadata!(@get_type $hp $pty),
            desc:        $crate::__plugin_metadata!(@get_dsc $dsc),
            full_desc:   $crate::__plugin_metadata!(@get_fd $mf $doc),
        }
    };
}

// ─── Internal helper: build Vec<ServiceEntry> from provides { … } content ────

/// Internal helper: builds `Vec<ServiceEntry>` from the raw token content of
/// `provides { Trait: Impl, … }`.
#[macro_export]
#[doc(hidden)]
macro_rules! __alloy_service_entries {
    // Empty provides
    () => {
        ::std::vec![]
    };

    // One or more `Trait: Impl` pairs
    ( $( $svc:path : $impl:ty ),+ $(,)? ) => {
        ::std::vec![$( $crate::plugin::ServiceEntry {
            id:      <dyn $svc as $crate::plugin::ServiceMeta>::ID,
            type_id: ::std::any::TypeId::of::<dyn $svc>(),
            factory: ::std::sync::Arc::new(
                |ctx: ::std::sync::Arc<$crate::plugin::PluginLoadContext>| {
                    ::std::boxed::Box::pin(async move {
                        let impl_val =
                            <$impl as $crate::plugin::ServiceInit>::init(ctx).await;
                        // Upcast to Arc<dyn ServiceTrait>
                        let trait_arc: ::std::sync::Arc<dyn $svc> =
                            ::std::sync::Arc::new(impl_val);
                        // Wrap so the stored type (Arc<dyn Trait>) is recoverable
                        // via downcast_ref::<Arc<dyn Trait>>() in get_service().
                        ::std::sync::Arc::new(trait_arc)
                            as ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>
                    })
                },
            ),
        } ),+]
    };
}

/// Internal helper: const `&[&str]` of service IDs from `provides { … }` content.
#[macro_export]
#[doc(hidden)]
macro_rules! __alloy_provides_ids {
    () => {
        &[] as &[&'static str]
    };
    ( $( $svc:path : $impl:ty ),+ $(,)? ) => {
        &[$( <dyn $svc as $crate::plugin::ServiceMeta>::ID ),+] as &[&'static str]
    };
}

/// Internal helper: const `&[&str]` of dependency IDs from `depends_on [ … ]` content.
#[macro_export]
#[doc(hidden)]
macro_rules! __alloy_depends_on_ids {
    () => {
        &[] as &[&'static str]
    };
    ( $( $dep:path ),+ $(,)? ) => {
        &[$( <dyn $dep as $crate::plugin::ServiceMeta>::ID ),+] as &[&'static str]
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
/// pub static MY_PLUGIN: PluginDescriptor = define_plugin! {
///     name: "my_plugin",
///
///     // Map of service trait → concrete implementation
///     provides: {
///         MyService: MyServiceImpl,
///     },
///
///     // Service traits required before this plugin loads
///     depends_on: [MyService],
///
///     handlers: [
///         on_message().handler(log_handler),
///         on_command::<EchoCommand>("echo").handler(echo_handler),
///     ],
///
///     on_load:   my_on_load,    // async fn(Arc<PluginLoadContext>) -> anyhow::Result<()>
///     on_unload: my_on_unload,  // async fn()
///
///     metadata: {
///         version:     "2.0.0",
///         desc:        "Short description.",
///         plugin_type: service,   // or `runtime`; auto-inferred when omitted
///     },
/// };
/// ```
///
/// ## Field reference
///
/// | Field | Required | Description |
/// |-------|----------|-------------|
/// | `name` | ✓ | Must be **first**. Plugin display name and config-section key. |
/// | `provides` | — | `{ Trait: ImplType, … }` — services injected at load time |
/// | `depends_on` | — | `[Trait, …]` — traits required before this plugin loads |
/// | `handlers` | — | `[expr, …]` — Tower handler services |
/// | `on_load` | — | `async fn(Arc<PluginLoadContext>) -> Result<()>` |
/// | `on_unload` | — | `async fn()` |
/// | `metadata` | — | `{ version, desc, full_desc, plugin_type }` |
///
/// [`PluginDescriptor`]: crate::plugin::PluginDescriptor
#[macro_export]
macro_rules! define_plugin {
    // ── Entry: with doc comment ───────────────────────────────────────────────
    //
    // Accumulator slots (8 total):
    //   [$n]         plugin name literal
    //   [$($pvs)*]   raw token content of provides { … }
    //   $hpvs        [] initially; [yes] once provides: is seen  (single tt)
    //   [$($dep)*]   raw token content of depends_on [ … ]
    //   [$($h),*]    handler expressions
    //   [$($lo)?]    on_load path
    //   [$($un)?]    on_unload path
    //   [$($doc)?]   doc literal
    //
    // $hpvs is carried as a plain `tt` (bracket group) so it can be forwarded
    // directly into __plugin_metadata! without any inner macro_rules! trickery.

    ($(#[doc = $doc:literal])+ name: $name:literal, $($tail:tt)+) => {
        $crate::define_plugin!(
            @acc [$name] [] [] [] [] [] [] [::std::concat!($($doc, " "),*)]
            $($tail)+
        )
    };

    // ── Entry: no doc + more fields ───────────────────────────────────────────
    (name: $name:literal, $($tail:tt)+) => {
        $crate::define_plugin!(
            @acc [$name] [] [] [] [] [] [] []
            $($tail)+
        )
    };

    // ── Entry: name only ──────────────────────────────────────────────────────
    (name: $name:literal $(,)?) => {
        $crate::define_plugin!(
            @acc [$name] [] [] [] [] [] [] []
        )
    };

    // ── Accumulator: skip stray commas ────────────────────────────────────────
    (@acc $n:tt $pvs:tt $hpvs:tt $dep:tt $h:tt $lo:tt $un:tt $doc:tt
        , $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc $n $pvs $hpvs $dep $h $lo $un $doc
            $($rest)*
        )
    };

    // ── Consume provides: { Trait: Impl, … } ────────────────────────────────────
    // Requires pvs slot to be empty (no duplicate provides:).
    // Sets $hpvs to [yes] unconditionally — presence of the key implies service.
    (
        @acc [$n:literal] [] $_hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        provides: { $($pvs:tt)* } $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] [yes] [$($dep)*] [$($h),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // ── Consume depends_on: [Trait, …] ───────────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        depends_on: [$($dep:tt)*] $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // ── Consume handlers: [expr, …] ───────────────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] $doc:tt
        handlers: [$($nh:expr),* $(,)?] $($rest:tt)*
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h,)* $($nh),*] [$($lo)?] [$($un)?] $doc
            $($rest)*
        )
    };

    // ── Consume on_load: path , <more fields> ─────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [] [$($un:expr)?] $doc:tt
        on_load: $lo:path , $($rest:tt)+
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$lo] [$($un)?] $doc
            $($rest)+
        )
    };

    // ── Consume on_load: path (last field) ────────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [] [$($un:expr)?] [$($doc:expr)?]
        on_load: $lo:path $(,)?
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$lo] [$($un)?] [$($doc)?]
        )
    };

    // ── Consume on_unload: path , <more fields> ───────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [] $doc:tt
        on_unload: $un:path , $($rest:tt)+
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$($lo)?] [$un] $doc
            $($rest)+
        )
    };

    // ── Consume on_unload: path (last field) ──────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [] [$($doc:expr)?]
        on_unload: $un:path $(,)?
    ) => {
        $crate::define_plugin!(
            @acc [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$($lo)?] [$un] [$($doc)?]
        )
    };

    // ── Consume metadata: { … } ───────────────────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] [$($doc:expr)?]
        metadata: { $($meta:tt)* } $(,)?
    ) => {
        $crate::define_plugin!(
            @terminal [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$($lo)?] [$($un)?]
                [$($doc)?] $($meta)*
        )
    };

    // ── No remaining fields → terminal ────────────────────────────────────────
    (
        @acc [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?] [$($doc:expr)?]
    ) => {
        $crate::define_plugin!(
            @terminal [$n] [$($pvs)*] $hpvs [$($dep)*] [$($h),*] [$($lo)?] [$($un)?]
                [$($doc)?]
        )
    };

    // ── @terminal — emit the PluginDescriptor ─────────────────────────────────
    //
    // Slots:
    //   [$n]          plugin name literal
    //   [$($pvs)*]    raw provides tokens: `T: I, T2: I2`
    //   $hpvs         [] (no provides) or [yes] (has provides) — single tt
    //   [$($dep)*]    raw depends_on tokens: `T, T2`
    //   [$($h),*]     handler expressions
    //   [$($lo)?]     on_load path (optional)
    //   [$($un)?]     on_unload path (optional)
    //   [$($doc)?]    doc literal (optional)
    //   $($meta)*     trailing metadata tokens
    //
    // $hpvs is passed directly to __plugin_metadata! as the has-provides flag;
    // no inner macro_rules! definition is needed.
    (
        @terminal [$n:literal] [$($pvs:tt)*] $hpvs:tt [$($dep:tt)*] [$($h:expr),*] [$($lo:expr)?] [$($un:expr)?]
            [$($doc:expr)?] $($meta:tt)*
    ) => {{
        const __ALLOY_PROVIDES_IDS: &[&str] = $crate::__alloy_provides_ids!($($pvs)*);
        const __ALLOY_DEPENDS_ON_IDS: &[&str] = $crate::__alloy_depends_on_ids!($($dep)*);

        const __ALLOY_META: $crate::plugin::PluginMetadata =
            $crate::__plugin_metadata!(
                @parse $hpvs [$($doc)?] $($meta)*
            );

        fn __alloy_plugin_create() -> $crate::plugin::Plugin {
            $crate::plugin::Plugin::__new(
                $n,
                {
                    let ids: &[&'static str] = $crate::__alloy_depends_on_ids!($($dep)*);
                    ids.to_vec()
                },
                vec![$( $crate::plugin::__BoxCloneSyncService::new($h) ),*],
                $crate::__alloy_service_entries!($($pvs)*),
                {
                    #[allow(unused_mut)]
                    let mut __f: ::std::option::Option<$crate::plugin::OnLoadFn> = None;
                    $(
                        __f = Some(::std::sync::Arc::new(
                            |ctx: ::std::sync::Arc<$crate::plugin::PluginLoadContext>| {
                                ::std::boxed::Box::pin(async move {
                                    $lo(ctx).await.map_err(
                                        |e| -> ::tower::BoxError { e.into() },
                                    )
                                })
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
