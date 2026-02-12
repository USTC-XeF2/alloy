//! Event derive macro implementation — **parent-in-child** design.
//!
//! # Overview
//!
//! `#[derive(BotEvent)]` generates:
//!
//! 1. `impl Event` — event metadata + delegation to parent
//! 2. `impl FromEvent` — JSON validation + deserialization
//! 3. `impl Deref[Mut]` — auto-generated when a parent field exists
//!
//! # Struct-level attributes `#[event(...)]`
//!
//! | Key | Example | Description |
//! |-----|---------|-------------|
//! | `name` | `"onebot.message.private"` | Full event name (default: `"{platform}.{snake_case_name}"`) |
//! | `platform` | `"onebot"` | Platform name (default: `"unknown"`) |
//! | `parent` | `"MessageEvent"` | Parent type ⟹ generates `Deref` / `DerefMut` |
//! | `type` | `"message"` | `EventType` variant |
//!
//! # Field-level attributes `#[event(...)]`
//!
//! | Key | Description |
//! |-----|-------------|
//! | `parent` | Marks this field as the parent (must be the type in `parent = "…"`) |
//! | `raw_json` | Field that stores `Option<Arc<str>>` of raw JSON |
//! | `bot_id` | Field that stores `Option<Arc<str>>` of bot ID |

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, spanned::Spanned};

// ============================================================================
// Attribute structures
// ============================================================================

/// Top-level `#[event(…)]` attributes.
#[derive(Default)]
pub struct EventAttrs {
    pub platform: Option<String>,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub event_type: Option<String>,
}

/// Per-field `#[event(…)]` markers.
#[derive(Default)]
struct FieldAttrs {
    is_parent: bool,
    is_raw_json: bool,
    is_bot_id: bool,
}

// ============================================================================
// Entry point
// ============================================================================

pub fn derive_bot_event(input: &DeriveInput) -> syn::Result<TokenStream> {
    let attrs = parse_event_attrs(&input.attrs)?;
    let name = &input.ident;

    match &input.data {
        Data::Struct(data) => generate_struct_impl(name, &attrs, &data.fields),
        Data::Enum(_) => Err(syn::Error::new(
            input.span(),
            "BotEvent no longer supports enums. Use structs with a `parent` field instead.",
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

fn parse_event_attrs(attrs: &[Attribute]) -> syn::Result<EventAttrs> {
    let mut result = EventAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("event") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("platform") {
                result.platform = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("name") {
                result.name = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("parent") {
                result.parent = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("type") {
                result.event_type = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            }
            Ok(())
        })?;
    }

    Ok(result)
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
            } else if meta.path.is_ident("bot_id") {
                result.is_bot_id = true;
            }
            Ok(())
        })?;
    }

    Ok(result)
}

// ============================================================================
// Code generation — structs only
// ============================================================================

fn generate_struct_impl(
    name: &Ident,
    attrs: &EventAttrs,
    fields: &Fields,
) -> syn::Result<TokenStream> {
    let platform = attrs.platform.as_deref().unwrap_or("unknown");
    let full_name = attrs
        .name
        .clone()
        .unwrap_or_else(|| format!("{}.{}", platform, to_snake_case(&name.to_string())));
    let full_name_lit = syn::LitStr::new(&full_name, name.span());
    let platform_lit = syn::LitStr::new(platform, name.span());

    // Scan fields for markers
    let mut parent_field_ident: Option<Ident> = None;
    let mut raw_json_field: Option<Ident> = None;
    let mut bot_id_field: Option<Ident> = None;

    if let Fields::Named(named) = fields {
        for f in &named.named {
            let fa = parse_field_attrs(&f.attrs)?;
            let ident = f.ident.as_ref().unwrap();
            if fa.is_parent {
                parent_field_ident = Some(ident.clone());
            }
            if fa.is_raw_json {
                raw_json_field = Some(ident.clone());
            }
            if fa.is_bot_id {
                bot_id_field = Some(ident.clone());
            }
        }
    }

    // ── event_type ──
    let event_type_impl = match attrs.event_type.as_deref() {
        Some(t) => {
            let variant = match t.to_lowercase().as_str() {
                "message" => quote! { ::alloy_core::EventType::Message },
                "notice" => quote! { ::alloy_core::EventType::Notice },
                "request" => quote! { ::alloy_core::EventType::Request },
                "meta" | "meta_event" => quote! { ::alloy_core::EventType::Meta },
                _ => quote! { ::alloy_core::EventType::Other },
            };
            quote! { fn event_type(&self) -> ::alloy_core::EventType { #variant } }
        }
        None => quote! {},
    };

    // ── raw_json / bot_id / plain_text delegation ──
    let (raw_json_impl, bot_id_impl, plain_text_impl);

    if let Some(ref pf) = parent_field_ident {
        // Delegate to parent
        let parent_ty: syn::Type = syn::parse_str(
            attrs
                .parent
                .as_deref()
                .expect("parent field found but no parent = \"...\" attribute"),
        )?;

        raw_json_impl = quote! {
            fn raw_json(&self) -> Option<&str> {
                <#parent_ty as ::alloy_core::Event>::raw_json(&self.#pf)
            }
        };
        bot_id_impl = quote! {
            fn bot_id(&self) -> Option<&str> {
                <#parent_ty as ::alloy_core::Event>::bot_id(&self.#pf)
            }
        };
        plain_text_impl = quote! {
            fn plain_text(&self) -> String {
                <#parent_ty as ::alloy_core::Event>::plain_text(&self.#pf)
            }
        };
    } else {
        // Root event — use field attrs if present
        raw_json_impl = if let Some(ref rj) = raw_json_field {
            quote! {
                fn raw_json(&self) -> Option<&str> {
                    self.#rj.as_deref()
                }
            }
        } else {
            quote! {}
        };
        bot_id_impl = if let Some(ref bi) = bot_id_field {
            quote! {
                fn bot_id(&self) -> Option<&str> {
                    self.#bi.as_deref()
                }
            }
        } else {
            quote! {}
        };
        // Root event plain_text returns empty string by default
        plain_text_impl = quote! {
            fn plain_text(&self) -> String {
                String::new()
            }
        };
    }

    // ── Deref / DerefMut ──
    let deref_impls = if let Some(ref pf) = parent_field_ident {
        let parent_ty: syn::Type = syn::parse_str(attrs.parent.as_deref().unwrap())?;
        quote! {
            impl ::std::ops::Deref for #name {
                type Target = #parent_ty;
                #[inline]
                fn deref(&self) -> &Self::Target {
                    &self.#pf
                }
            }

            impl ::std::ops::DerefMut for #name {
                #[inline]
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.#pf
                }
            }
        }
    } else {
        quote! {}
    };

    // ── FromEvent — simple downcast/parse (no validation) ──
    let from_event_impl = quote! {
        impl ::alloy_core::FromEvent for #name {
            fn from_event(root: &dyn ::alloy_core::Event) -> Option<Self> {
                // 1. Downcast
                if let Some(e) = root.as_any().downcast_ref::<Self>() {
                    return Some(e.clone());
                }
                // 2. Parse from raw JSON
                let json = root.raw_json()?;
                ::serde_json::from_str(json).ok()
            }
        }
    };

    // ── Event trait impl ──
    let event_impl = quote! {
        impl ::alloy_core::Event for #name {
            fn event_name(&self) -> &'static str {
                #full_name_lit
            }

            fn platform(&self) -> &'static str {
                #platform_lit
            }

            #event_type_impl

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            #raw_json_impl
            #bot_id_impl
            #plain_text_impl
        }
    };

    Ok(quote! {
        #deref_impls
        #event_impl
        #from_event_impl
    })
}

// ============================================================================
// Utility
// ============================================================================

/// Converts `CamelCase` → `snake_case`.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}
