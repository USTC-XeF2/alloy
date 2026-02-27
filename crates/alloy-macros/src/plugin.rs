use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Expr, Ident, ItemTrait, LitStr, Path, Token, Type, braced, bracketed,
    parse::{Parse, ParseStream, Result},
    punctuated::Punctuated,
};

// ─── Input AST types ─────────────────────────────────────────────────────────

/// One `Trait: ImplType` entry in `provides: { … }`.
struct ProvidesEntry {
    trait_path: Path,
    impl_type: Type,
}

/// Optional overrides from `metadata: { … }`.
#[derive(Default)]
struct MetadataOpts {
    version: Option<LitStr>,
    desc: Option<LitStr>,
    full_desc: Option<LitStr>,
    plugin_type: Option<Ident>, // "service" | "runtime"
}

/// Parsed content of the whole `define_plugin! { … }` invocation.
pub struct DefinePluginInput {
    /// Leading `/// …` doc attributes, in order.
    doc_attrs: Vec<Attribute>,
    name: LitStr,
    provides: Vec<ProvidesEntry>,
    depends_on: Vec<Path>,
    handlers: Vec<Expr>,
    on_load: Option<Path>,
    on_unload: Option<Path>,
    metadata: MetadataOpts,
}

// ─── Parsing ──────────────────────────────────────────────────────────────────

/// Parse `{ Trait: ImplType, … }`.
fn parse_provides(input: ParseStream) -> Result<Vec<ProvidesEntry>> {
    let content;
    braced!(content in input);
    let mut entries = Vec::new();
    while !content.is_empty() {
        while content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
        if content.is_empty() {
            break;
        }
        let trait_path = content.parse()?;
        content.parse::<Token![:]>()?;
        let impl_type: Type = content.parse()?;
        entries.push(ProvidesEntry {
            trait_path,
            impl_type,
        });
    }
    Ok(entries)
}

/// Parse `[ Trait, … ]`.
fn parse_depends_on(input: ParseStream) -> Result<Vec<Path>> {
    let content;
    bracketed!(content in input);
    let paths: Punctuated<_, Token![,]> =
        content.parse_terminated(Path::parse, Token![,])?;
    Ok(paths.into_iter().collect())
}

/// Parse `[ expr, … ]`.
fn parse_handlers(input: ParseStream) -> Result<Vec<Expr>> {
    let content;
    bracketed!(content in input);
    let exprs: Punctuated<Expr, Token![,]> = content.parse_terminated(Expr::parse, Token![,])?;
    Ok(exprs.into_iter().collect())
}

/// Parse `{ version: "…", desc: "…", full_desc: "…", plugin_type: service|runtime }`.
fn parse_metadata(input: ParseStream) -> Result<MetadataOpts> {
    let content;
    braced!(content in input);
    let mut opts = MetadataOpts::default();
    while !content.is_empty() {
        while content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
        if content.is_empty() {
            break;
        }
        let key: Ident = content.parse()?;
        content.parse::<Token![:]>()?;
        match key.to_string().as_str() {
            "version" => opts.version = Some(content.parse()?),
            "desc" => opts.desc = Some(content.parse()?),
            "full_desc" => opts.full_desc = Some(content.parse()?),
            "plugin_type" => opts.plugin_type = Some(content.parse()?),
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "unknown metadata key `{other}`; expected version, desc, full_desc, or plugin_type"
                    ),
                ));
            }
        }
    }
    Ok(opts)
}

impl Parse for DefinePluginInput {
    fn parse(input: ParseStream) -> Result<Self> {
        // ── Optional leading doc attributes: `/// …`  ─────────────────────────
        // `///` comments are expanded to `#[doc = "…"]` before macro input.
        let doc_attrs = Attribute::parse_outer(input)?;
        for attr in &doc_attrs {
            if !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "only `/// …` doc attributes are allowed before `name:`",
                ));
            }
        }

        // ── Required: name: "…"  ─────────────────────────────────────────────
        let name_kw: Ident = input.parse()?;
        if name_kw != "name" {
            return Err(syn::Error::new(
                name_kw.span(),
                "define_plugin! must start with `name: \"…\"`",
            ));
        }
        input.parse::<Token![:]>()?;
        let name: LitStr = input.parse()?;

        let mut out = DefinePluginInput {
            doc_attrs,
            name,
            provides: Vec::new(),
            depends_on: Vec::new(),
            handlers: Vec::new(),
            on_load: None,
            on_unload: None,
            metadata: MetadataOpts::default(),
        };

        // ── Optional fields in any order ──────────────────────────────────────
        loop {
            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
            if input.is_empty() {
                break;
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "provides" => out.provides = parse_provides(input)?,
                "depends_on" => out.depends_on = parse_depends_on(input)?,
                "handlers" => out.handlers = parse_handlers(input)?,
                "on_load" => out.on_load = Some(input.parse()?),
                "on_unload" => out.on_unload = Some(input.parse()?),
                "metadata" => out.metadata = parse_metadata(input)?,
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown field `{other}`; expected name, provides, depends_on, handlers, on_load, on_unload, or metadata"
                        ),
                    ));
                }
            }
        }
        Ok(out)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// `"echo_bot"` → `ECHO_BOT_PLUGIN`  (uppercased, `-` → `_`, with `_PLUGIN` suffix).
fn name_to_static_ident(name: &LitStr) -> Ident {
    let upper = name.value().to_uppercase().replace('-', "_");
    let with_suffix = format!("{}_PLUGIN", upper);
    Ident::new(&with_suffix, Span::call_site())
}

/// Extract the text of `#[doc = "…"]` attributes and join with newlines.
/// Returns `None` when there are no doc attrs.
fn doc_attrs_to_string(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if attr.path().is_ident("doc") {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        return Some(s.value().trim().to_owned());
                    }
                }
            }
            None
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

// ─── Code generation ──────────────────────────────────────────────────────────

pub fn expand(input: DefinePluginInput) -> TokenStream {
    let DefinePluginInput {
        doc_attrs,
        name,
        provides,
        depends_on,
        handlers,
        on_load,
        on_unload,
        metadata,
    } = input;

    let fw = quote! { ::alloy::framework };

    // ── Static variable name ──────────────────────────────────────────────────
    let static_ident = name_to_static_ident(&name);

    // ── provides IDs (static slice) ───────────────────────────────────────────
    let provides_ids = provides.iter().map(|e| {
        let t = &e.trait_path;
        quote! { <dyn #t as #fw::plugin::ServiceMeta>::ID }
    });
    let provides_ids_tokens = quote! { &[ #( #provides_ids ),* ] };

    // ── depends_on IDs (static slice) ─────────────────────────────────────────
    let dep_ids = depends_on.iter().map(|p| {
        quote! { <dyn #p as #fw::plugin::ServiceMeta>::ID }
    });
    let depends_on_ids_tokens = quote! { &[ #( #dep_ids ),* ] };

    // ── metadata: full_desc — explicit beats doc, doc beats None ──────────────
    let full_desc_tokens = if let Some(fd) = &metadata.full_desc {
        quote! { ::std::option::Option::Some(#fd) }
    } else if let Some(doc_text) = doc_attrs_to_string(&doc_attrs) {
        quote! { ::std::option::Option::Some(#doc_text) }
    } else {
        quote! { ::std::option::Option::None }
    };

    let version_tokens = match &metadata.version {
        Some(v) => quote! { #v },
        None => quote! { ::std::env!("CARGO_PKG_VERSION") },
    };
    let desc_tokens = match &metadata.desc {
        Some(d) => quote! { #d },
        None => quote! { ::std::env!("CARGO_PKG_DESCRIPTION") },
    };
    let plugin_type_tokens = match &metadata.plugin_type {
        Some(pt) if pt == "service" => quote! { #fw::plugin::PluginType::Service },
        Some(pt) if pt == "runtime" => quote! { #fw::plugin::PluginType::Runtime },
        Some(other) => {
            return syn::Error::new(other.span(), "plugin_type must be `service` or `runtime`")
                .to_compile_error();
        }
        None => {
            if provides.is_empty() {
                quote! { #fw::plugin::PluginType::Runtime }
            } else {
                quote! { #fw::plugin::PluginType::Service }
            }
        }
    };

    // ── ServiceEntry vec ──────────────────────────────────────────────────────
    let service_entries = provides.iter().map(|e| {
        let t = &e.trait_path;
        let i = &e.impl_type;
        quote! {
            #fw::plugin::ServiceEntry {
                id:      <dyn #t as #fw::plugin::ServiceMeta>::ID,
                type_id: ::std::any::TypeId::of::<dyn #t>(),
                factory: ::std::sync::Arc::new(
                    |ctx: ::std::sync::Arc<#fw::plugin::PluginLoadContext>| {
                        ::std::boxed::Box::pin(async move {
                            let impl_val = <#i as #fw::plugin::ServiceInit>::init(ctx).await;
                            let trait_arc: ::std::sync::Arc<dyn #t> =
                                ::std::sync::Arc::new(impl_val);
                            ::std::sync::Arc::new(trait_arc)
                                as ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>
                        })
                    },
                ),
            }
        }
    });

    // ── depends_on vec (runtime) ──────────────────────────────────────────────
    let dep_id_vecs = depends_on.iter().map(|p| {
        quote! { <dyn #p as #fw::plugin::ServiceMeta>::ID }
    });

    // ── handler vec ───────────────────────────────────────────────────────────
    let handler_entries = handlers.iter().map(|h| {
        quote! { #fw::plugin::__BoxCloneSyncService::new(#h) }
    });

    // ── on_load / on_unload closures ──────────────────────────────────────────
    let on_load_tokens = match &on_load {
        Some(f) => quote! {
            ::std::option::Option::Some(::std::boxed::Box::new(
                |ctx: ::std::sync::Arc<#fw::plugin::PluginLoadContext>| {
                    ::std::boxed::Box::pin(async move {
                        #f(ctx).await.map_err(|e| -> ::tower::BoxError { e.into() })
                    })
                },
            ))
        },
        None => quote! { ::std::option::Option::None },
    };
    let on_unload_tokens = match &on_unload {
        Some(f) => quote! {
            ::std::option::Option::Some(::std::boxed::Box::new(
                || ::std::boxed::Box::pin(#f()),
            ))
        },
        None => quote! { ::std::option::Option::None },
    };

    // ── Final expansion: emit a `pub static` item ─────────────────────────────
    quote! {
        #(#doc_attrs)*
        pub static #static_ident: #fw::plugin::PluginDescriptor = {
            const __ALLOY_PROVIDES_IDS:  &[&'static str] = #provides_ids_tokens;
            const __ALLOY_DEPENDS_ON_IDS: &[&'static str] = #depends_on_ids_tokens;

            const __ALLOY_META: #fw::plugin::PluginMetadata = #fw::plugin::PluginMetadata {
                version:     #version_tokens,
                plugin_type: #plugin_type_tokens,
                desc:        #desc_tokens,
                full_desc:   #full_desc_tokens,
            };

            fn __alloy_plugin_create() -> #fw::plugin::Plugin {
                #fw::plugin::Plugin::__new(
                    #name,
                    vec![ #( #dep_id_vecs ),* ],
                    vec![ #( #handler_entries ),* ],
                    vec![ #( #service_entries ),* ],
                    #on_load_tokens,
                    #on_unload_tokens,
                    __ALLOY_META,
                )
            }

            #fw::plugin::PluginDescriptor {
                api_version: #fw::plugin::ALLOY_PLUGIN_API_VERSION,
                name:        #name,
                provides:    __ALLOY_PROVIDES_IDS,
                depends_on:  __ALLOY_DEPENDS_ON_IDS,
                create:      __alloy_plugin_create,
                metadata:    __ALLOY_META,
            }
        };
    }
}

// ─── service_meta macro implementation ──────────────────────────────────────

/// Parses the service_meta macro input (e.g., `"storage"`)
pub fn expand_service_meta(attr: TokenStream, item: TokenStream) -> TokenStream {
    let id: LitStr = match syn::parse2(attr) {
        Ok(id) => id,
        Err(err) => return err.to_compile_error().into(),
    };

    let item_trait: ItemTrait = match syn::parse2(item) {
        Ok(trait_item) => trait_item,
        Err(err) => return err.to_compile_error().into(),
    };

    let trait_name = &item_trait.ident;

    // Output the trait unchanged, plus the ServiceMeta impl
    let expanded = quote! {
        #item_trait

        impl ::alloy::framework::plugin::ServiceMeta for dyn #trait_name {
            const ID: &'static str = #id;
        }
    };

    expanded.into()
}
