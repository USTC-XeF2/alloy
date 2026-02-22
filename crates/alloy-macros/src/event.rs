//! Event derive macro implementation — **parent-in-child** design.
//!
//! # Overview
//!
//! `#[derive(BotEvent)]` generates:
//!
//! 1. `impl Event` — event metadata + downgrade_any method for parent chain traversal
//! 2. `impl Deref[Mut]` — auto-generated when a parent field exists
//!
//! # Root events: `#[root_event(...)]`
//!
//! Used for the top-level event of a platform. It has no parent and defines
//! the platform name and segment type that all child events will inherit.
//!
//! | Key | Example | Required | Description |
//! |-----|---------|----------|-------------|
//! | `platform` | `"onebot"` | **Yes** | Platform name; also used as event name |
//! | `segment_type` | `"crate::segment::Segment"` | **Yes** | Segment type for the whole platform |
//!
//! # Child events: `#[event(...)]`
//!
//! Used for all non-root events. The parent is detected from the field
//! marked with `#[event(parent)]`, so you only need to write it once.
//!
//! | Key | Example | Required | Description |
//! |-----|---------|----------|-------------|
//! | `name` | `"message.private"` | No | Event name suffix (auto-prefixed with `{platform}.`) |
//! | `type` | `"message"` | No | `EventType` variant (default: inherited from parent or `Other`) |
//!
//! # Field-level attributes `#[event(...)]`
//!
//! | Key | Description |
//! |-----|-------------|
//! | `parent` | Marks this field as the parent (type is auto-detected) |
//! | `raw_json` | Field that stores `Option<Arc<str>>` of raw JSON |
//! | `bot_id` | Field that stores `Option<Arc<str>>` of bot ID |
//! | `message` | Field of type `Message<Segment>`, used for `Event::get_message()` |

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type, spanned::Spanned};

// ============================================================================
// Attribute structures
// ============================================================================

/// Which kind of struct-level attribute was found.
enum EventKind {
    /// `#[root_event(platform = "…", segment_type = "…")]`
    Root {
        platform: String,
        segment_type: String,
    },
    /// `#[event(name = "…", type = "…")]`
    Child {
        name: Option<String>,
        event_type: Option<String>,
    },
}

/// Per-field `#[event(…)]` markers.
#[derive(Default)]
struct FieldAttrs {
    is_parent: bool,
    is_raw_json: bool,
    is_message: bool,
}

// ============================================================================
// Entry point
// ============================================================================

pub fn derive_bot_event(input: &DeriveInput) -> syn::Result<TokenStream> {
    let kind = parse_struct_attrs(&input.attrs, input.ident.span())?;
    let name = &input.ident;

    match &input.data {
        Data::Struct(data) => generate_struct_impl(name, &kind, &data.fields),
        Data::Enum(_) => Err(syn::Error::new(
            input.span(),
            "BotEvent does not support enums. Use structs with a parent field instead.",
        )),
        Data::Union(_) => Err(syn::Error::new(
            input.span(),
            "BotEvent cannot be derived for unions",
        )),
    }
}

// ============================================================================
// Attribute parsing
// ============================================================================

fn parse_struct_attrs(attrs: &[Attribute], span: proc_macro2::Span) -> syn::Result<EventKind> {
    // Check for #[root_event(...)]
    for attr in attrs {
        if attr.path().is_ident("root_event") {
            let mut platform: Option<String> = None;
            let mut segment_type: Option<String> = None;

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("platform") {
                    platform = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("segment_type") {
                    segment_type = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            })?;

            let platform = platform.ok_or_else(|| {
                syn::Error::new(span, "#[root_event] requires `platform = \"…\"`")
            })?;
            let segment_type = segment_type.ok_or_else(|| {
                syn::Error::new(span, "#[root_event] requires `segment_type = \"…\"`")
            })?;

            return Ok(EventKind::Root {
                platform,
                segment_type,
            });
        }
    }

    // Check for #[event(...)]
    for attr in attrs {
        if attr.path().is_ident("event") {
            let mut name: Option<String> = None;
            let mut event_type: Option<String> = None;

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    name = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("type") {
                    event_type = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            })?;

            return Ok(EventKind::Child { name, event_type });
        }
    }

    Err(syn::Error::new(
        span,
        "BotEvent requires either #[root_event(...)] or #[event(...)] attribute",
    ))
}

fn parse_field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut result = FieldAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("event") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("parent") {
                result.is_parent = true;
            } else if meta.path.is_ident("raw_json") {
                result.is_raw_json = true;
            } else if meta.path.is_ident("message") {
                result.is_message = true;
            }
            Ok(())
        })?;
    }

    Ok(result)
}

// ============================================================================
// Code generation
// ============================================================================

fn generate_struct_impl(
    name: &Ident,
    kind: &EventKind,
    fields: &Fields,
) -> syn::Result<TokenStream> {
    // Scan fields for markers
    let mut parent_field: Option<(Ident, Type)> = None;
    let mut raw_json_field: Option<Ident> = None;
    let mut message_field: Option<(Ident, Type)> = None;

    if let Fields::Named(named) = fields {
        for f in &named.named {
            let fa = parse_field_attrs(&f.attrs)?;
            let ident = f.ident.as_ref().unwrap();
            if fa.is_parent {
                parent_field = Some((ident.clone(), f.ty.clone()));
            }
            if fa.is_raw_json {
                raw_json_field = Some(ident.clone());
            }
            if fa.is_message {
                message_field = Some((ident.clone(), f.ty.clone()));
            }
        }
    }

    match kind {
        EventKind::Root {
            platform,
            segment_type,
        } => {
            if parent_field.is_some() {
                return Err(syn::Error::new(
                    name.span(),
                    "#[root_event] must not have a #[event(parent)] field",
                ));
            }
            generate_root_event(name, platform, segment_type, raw_json_field, message_field)
        }
        EventKind::Child {
            name: event_name,
            event_type,
        } => {
            let (pf_ident, pf_ty) = parent_field.ok_or_else(|| {
                syn::Error::new(
                    name.span(),
                    "#[event] requires a field marked with #[event(parent)]",
                )
            })?;
            Ok(generate_child_event(
                name,
                event_name.as_deref(),
                event_type.as_deref(),
                &pf_ident,
                &pf_ty,
                message_field,
            ))
        }
    }
}

// ============================================================================
// Root event generation
// ============================================================================

fn generate_root_event(
    name: &Ident,
    platform: &str,
    segment_type_str: &str,
    raw_json_field: Option<Ident>,
    message_field: Option<(Ident, Type)>,
) -> syn::Result<TokenStream> {
    let platform_lit = syn::LitStr::new(platform, name.span());
    let seg_ty: Type = syn::parse_str(segment_type_str)?;

    let raw_json_impl = if let Some(rj) = raw_json_field {
        quote! {
            fn raw_json(&self) -> Option<&str> {
                self.#rj.as_deref()
            }
        }
    } else {
        quote! {}
    };

    let (segment_type_impl, get_message_impl);
    if let Some((mf, _)) = message_field {
        segment_type_impl = quote! { type Segment = #seg_ty; };
        get_message_impl = quote! {
            fn get_message(&self) -> &::alloy_core::Message<Self::Segment> where Self: Sized {
                &self.#mf
            }
        };
    } else {
        segment_type_impl = quote! { type Segment = #seg_ty; };
        get_message_impl = quote! {
            fn get_message(&self) -> &::alloy_core::Message<Self::Segment> where Self: Sized {
                static EMPTY: ::std::sync::OnceLock<::alloy_core::Message<#seg_ty>> = ::std::sync::OnceLock::new();
                EMPTY.get_or_init(|| ::alloy_core::Message::new())
            }
        };
    }

    let downgrade_any_impl = quote! {
        fn downgrade_any(&self, type_id: ::std::any::TypeId) -> Option<Box<dyn ::std::any::Any>> {
            // Root event: only matches self
            if type_id == ::std::any::TypeId::of::<Self>() {
                Some(Box::new(self.clone()))
            } else {
                None
            }
        }
    };

    let event_impl = quote! {
        impl ::alloy_core::Event for #name {
            fn event_name(&self) -> &'static str {
                #platform_lit
            }

            fn platform(&self) -> &'static str {
                #platform_lit
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            #downgrade_any_impl
            #raw_json_impl
            #segment_type_impl
            #get_message_impl
        }
    };

    Ok(quote! {
        #event_impl
    })
}

// ============================================================================
// Child event generation
// ============================================================================

fn generate_child_event(
    name: &Ident,
    event_name: Option<&str>,
    event_type: Option<&str>,
    parent_field_ident: &Ident,
    parent_ty: &Type,
    message_field: Option<(Ident, Type)>,
) -> TokenStream {
    // ── event_type ──
    let event_type_impl = match event_type {
        Some(t) => {
            let variant = match t.to_lowercase().as_str() {
                "message" => quote! { ::alloy_core::EventType::Message },
                "notice" => quote! { ::alloy_core::EventType::Notice },
                "request" => quote! { ::alloy_core::EventType::Request },
                "meta" => quote! { ::alloy_core::EventType::Meta },
                _ => quote! { ::alloy_core::EventType::Other },
            };
            quote! { fn event_type(&self) -> ::alloy_core::EventType { #variant } }
        }
        None => {
            quote! {
                fn event_type(&self) -> ::alloy_core::EventType {
                    <#parent_ty as ::alloy_core::Event>::event_type(&self.#parent_field_ident)
                }
            }
        }
    };

    // ── event_name ──
    // For child events with a name suffix, we build "{platform}.{suffix}" at first call
    // using OnceLock + Box::leak to get a &'static str.
    // For child events without a name, we delegate to parent.
    let event_name_impl = if let Some(suffix) = event_name {
        let suffix_lit = syn::LitStr::new(suffix, name.span());
        quote! {
            fn event_name(&self) -> &'static str {
                static FULL_NAME: ::std::sync::OnceLock<String> = ::std::sync::OnceLock::new();
                FULL_NAME.get_or_init(|| {
                    let platform = <#parent_ty as ::alloy_core::Event>::platform(&self.#parent_field_ident);
                    format!("{}.{}", platform, #suffix_lit)
                })
            }
        }
    } else {
        quote! {
            fn event_name(&self) -> &'static str {
                <#parent_ty as ::alloy_core::Event>::event_name(&self.#parent_field_ident)
            }
        }
    };

    // ── platform — always delegate to parent ──
    let platform_impl = quote! {
        fn platform(&self) -> &'static str {
            <#parent_ty as ::alloy_core::Event>::platform(&self.#parent_field_ident)
        }
    };

    // ── raw_json — always delegate to parent ──
    let raw_json_impl = quote! {
        fn raw_json(&self) -> Option<&str> {
            <#parent_ty as ::alloy_core::Event>::raw_json(&self.#parent_field_ident)
        }
    };

    // ── message type / get_message / get_plain_text ──
    let (segment_type_impl, get_message_impl);
    if let Some((mf, _)) = message_field {
        segment_type_impl = quote! {
            type Segment = <#parent_ty as ::alloy_core::Event>::Segment;
        };
        get_message_impl = quote! {
            fn get_message(&self) -> &::alloy_core::Message<Self::Segment> where Self: Sized {
                &self.#mf
            }
        };
    } else {
        segment_type_impl = quote! {
            type Segment = <#parent_ty as ::alloy_core::Event>::Segment;
        };
        get_message_impl = quote! {
            fn get_message(&self) -> &::alloy_core::Message<Self::Segment> where Self: Sized {
                <#parent_ty as ::alloy_core::Event>::get_message(&self.#parent_field_ident)
            }
        };
    }

    // ── Deref / DerefMut ──
    let deref_impls = quote! {
        impl ::std::ops::Deref for #name {
            type Target = #parent_ty;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.#parent_field_ident
            }
        }

        impl ::std::ops::DerefMut for #name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.#parent_field_ident
            }
        }
    };

    // ── DowngradeAny ──
    let downgrade_any_impl = quote! {
        fn downgrade_any(&self, type_id: ::std::any::TypeId) -> Option<Box<dyn ::std::any::Any>> {
            // Check if it's self first
            if type_id == ::std::any::TypeId::of::<Self>() {
                return Some(Box::new(self.clone()));
            }
            // Delegate to parent
            <#parent_ty as ::alloy_core::Event>::downgrade_any(&self.#parent_field_ident, type_id)
        }
    };

    // ── Event trait impl ──
    let event_impl = quote! {
        impl ::alloy_core::Event for #name {
            #event_name_impl
            #platform_impl
            #event_type_impl

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            #downgrade_any_impl
            #raw_json_impl
            #segment_type_impl
            #get_message_impl
        }
    };

    quote! {
        #deref_impls
        #event_impl
    }
}
