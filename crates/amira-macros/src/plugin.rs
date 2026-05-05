use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Expr, Ident, ItemTrait, LitStr, Path, Token, Type, braced, bracketed,
    parse::{Parse, ParseStream, Result},
    punctuated::Punctuated,
};

// ─── Input AST types ─────────────────────────────────────────────────────────

/// One `Trait: ImplType` entry in `provides: { … }`, with optional `#[cfg(...)]` attributes.
struct ProvidesEntry {
    attrs: Vec<Attribute>,
    trait_path: Path,
    impl_type: Type,
}

/// One dependency entry in `depends_on: [ … ]`, with optional `#[cfg(...)]` attributes.
/// If prefixed with `!`, the dependency is required; otherwise, it's optional.
struct DependsOnEntry {
    attrs: Vec<Attribute>,
    path: Path,
    is_required: bool,
}

/// Parsed content of the whole `define_plugin! { … }` invocation.
pub struct DefinePluginInput {
    /// Leading `/// …` doc attributes, in order.
    doc_attrs: Vec<Attribute>,
    name: LitStr,
    version: Option<LitStr>,
    provides: Vec<ProvidesEntry>,
    depends_on: Vec<DependsOnEntry>,
    handlers: Vec<Expr>,
    on_load: Option<Path>,
    on_unload: Option<Path>,
}

// ─── Parsing ──────────────────────────────────────────────────────────────────

/// Parse `{ Trait: ImplType, … }` with optional `#[cfg(...)]` attributes.
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
        let attrs = Attribute::parse_outer(&content)?;
        let trait_path = content.parse()?;
        content.parse::<Token![:]>()?;
        let impl_type: Type = content.parse()?;
        entries.push(ProvidesEntry {
            attrs,
            trait_path,
            impl_type,
        });
    }
    Ok(entries)
}

/// Parse `[ Trait, … ]` or `[ !Trait, … ]` with optional `#[cfg(...)]` attributes.
/// Entries prefixed with `!` are required; otherwise optional.
fn parse_depends_on(input: ParseStream) -> Result<Vec<DependsOnEntry>> {
    let content;
    bracketed!(content in input);
    let mut entries = Vec::new();
    while !content.is_empty() {
        while content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
        if content.is_empty() {
            break;
        }
        let attrs = Attribute::parse_outer(&content)?;

        // Check for optional `!` prefix
        let is_required = if content.peek(Token![!]) {
            content.parse::<Token![!]>()?;
            true
        } else {
            false
        };

        let path = content.parse()?;
        entries.push(DependsOnEntry {
            attrs,
            path,
            is_required,
        });
    }
    Ok(entries)
}

/// Parse `[ expr, … ]`.
fn parse_handlers(input: ParseStream) -> Result<Vec<Expr>> {
    let content;
    bracketed!(content in input);
    let exprs: Punctuated<Expr, Token![,]> = content.parse_terminated(Expr::parse, Token![,])?;
    Ok(exprs.into_iter().collect())
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
            version: None,
            provides: Vec::new(),
            depends_on: Vec::new(),
            handlers: Vec::new(),
            on_load: None,
            on_unload: None,
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
                "version" => out.version = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown field `{other}`; expected name, provides, depends_on, handlers, on_load, on_unload, or version"
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
    let with_suffix = format!("{upper}_PLUGIN");
    Ident::new(&with_suffix, Span::call_site())
}

/// Extract the text of `#[doc = "…"]` attributes and join with newlines.
/// Returns `None` when there are no doc attrs.
fn doc_attrs_to_string(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if let syn::Meta::NameValue(nv) = &attr.meta
                && attr.path().is_ident("doc")
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
            {
                return Some(s.value().trim().to_owned());
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
        version,
        provides,
        depends_on,
        handlers,
        on_load,
        on_unload,
    } = input;

    let fw = quote! { ::amira::framework };

    // ── Static variable name ──────────────────────────────────────────────────
    let static_ident = name_to_static_ident(&name);

    // ── depends_on IDs (static slice of DependsOnEntry) ──────────────────────
    let depends_on_entries: Vec<_> = depends_on
        .iter()
        .map(|e| {
            let p = &e.path;
            let attrs = &e.attrs;
            let required = e.is_required;
            quote! {
                #(#attrs)*
                #fw::plugin::DependsOnEntry {
                    name: <dyn #p as #fw::plugin::ServiceMeta>::ID,
                    required: #required,
                }
            }
        })
        .collect();
    let depends_on_tokens = quote! { &[ #( #depends_on_entries ),* ] };

    // ── metadata: desc — doc comment beats CARGO_PKG_DESCRIPTION ──────────────
    let desc_tokens = if let Some(doc_text) = doc_attrs_to_string(&doc_attrs) {
        quote! { #doc_text }
    } else {
        quote! { ::std::env!("CARGO_PKG_DESCRIPTION") }
    };

    let version_tokens = if let Some(v) = &version {
        quote! { #v }
    } else {
        quote! { ::std::env!("CARGO_PKG_VERSION") }
    };

    // ── ServiceEntry vec ──────────────────────────────────────────────────────
    let service_entries = provides.iter().map(|e| {
        let t = &e.trait_path;
        let i = &e.impl_type;
        let attrs = &e.attrs;
        quote! {
            #(#attrs)*
            #fw::plugin::ServiceEntry {
                id:      <dyn #t as #fw::plugin::ServiceMeta>::ID,
                type_id: ::std::any::TypeId::of::<dyn #t>(),
                factory: |ctx: ::std::sync::Arc<#fw::context::PluginContext>| {
                    ::std::boxed::Box::pin(async move {
                        match <#i as #fw::plugin::ServiceInit>::init(ctx).await {
                            ::std::result::Result::Ok(impl_val) => {
                                let trait_arc: ::std::sync::Arc<dyn #t> =
                                    ::std::sync::Arc::new(impl_val);
                                ::std::result::Result::Ok(::std::sync::Arc::new(trait_arc)
                                    as ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>)
                            }
                            ::std::result::Result::Err(e) => ::std::result::Result::Err(e),
                        }
                    })
                },
            }
        }
    });

    // ── handler vec ───────────────────────────────────────────────────────────
    let handler_entries = handlers.iter().map(|h| {
        quote! { #fw::plugin::__BoxCloneSyncService::new(#h) }
    });

    // ── on_load / on_unload closures ──────────────────────────────────────────
    let on_load_tokens = if let Some(f) = &on_load {
        quote! {
            ::std::option::Option::Some(|ctx: #fw::plugin::PluginLoadContext| {
                ::std::boxed::Box::pin(async move {
                    #f(ctx)
                        .await
                        .map_err(|e| -> ::std::boxed::Box<dyn ::std::fmt::Display + Send> {
                            ::std::boxed::Box::new(e)
                        })
                })
            })
        }
    } else {
        quote! { ::std::option::Option::None }
    };
    let on_unload_tokens = if let Some(f) = &on_unload {
        quote! {
            ::std::option::Option::Some(|| ::std::boxed::Box::pin(#f()))
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    let service_entries_tokens = quote! { &[ #( #service_entries ),* ] };

    // ── Final expansion: emit a `pub static` item ─────────────────────────────
    quote! {
        #(#doc_attrs)*
        pub static #static_ident: #fw::plugin::PluginDescriptor = #fw::plugin::PluginDescriptor {
            name:             #name,
            version:          #version_tokens,
            desc:             #desc_tokens,
            provides:         #service_entries_tokens,
            depends_on:       #depends_on_tokens,
            on_load:          #on_load_tokens,
            on_unload:        #on_unload_tokens,
            create_handlers:  || vec![ #( #handler_entries ),* ],
        };
    }
}

// ─── service_meta macro implementation ──────────────────────────────────────

/// Parses the service_meta macro input (e.g., `"storage"`)
pub fn expand_service_meta(attr: TokenStream, item: TokenStream) -> TokenStream {
    let id: LitStr = match syn::parse2(attr) {
        Ok(id) => id,
        Err(err) => return err.to_compile_error(),
    };

    let item_trait: ItemTrait = match syn::parse2(item) {
        Ok(trait_item) => trait_item,
        Err(err) => return err.to_compile_error(),
    };

    let trait_name = &item_trait.ident;

    // Output the trait unchanged, plus the ServiceMeta impl
    let expanded = quote! {
        #item_trait

        impl ::amira::framework::plugin::ServiceMeta for dyn #trait_name {
            const ID: &'static str = #id;
        }
    };

    expanded
}
