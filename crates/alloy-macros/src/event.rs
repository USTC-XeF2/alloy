//! Event derive macros implementation.
//!
//! This module provides derive macros for the event system:
//!
//! - `#[derive(BotEvent)]` - Generates Event trait and FromEvent implementations
//!
//! # Design Principles
//!
//! The macros are **completely platform-neutral** and do not assume any specific
//! field names like `self_id`, `time`, etc. All business logic belongs in the
//! adapter layer.
//!
//! # Attributes
//!
//! - `#[event(platform = "...")]` - Required. Specifies the platform name.
//! - `#[event(name = "...")]` - Optional. Override the auto-generated event name.
//! - `#[event(parent = "...")]` - Optional. Specifies parent type for hierarchical extraction.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Variant, spanned::Spanned};

// ============================================================================
// BotEvent Derive Macro
// ============================================================================

/// Attributes parsed from `#[event(...)]` on a struct or enum.
#[derive(Default)]
pub struct EventAttrs {
    /// Platform name (e.g., "onebot", "discord")
    pub platform: Option<String>,
    /// Event name override (full path like "onebot.message.private")
    pub name: Option<String>,
    /// Parent event type for FromEvent chaining
    pub parent: Option<String>,
}

/// Attributes parsed from `#[event(...)]` or `#[serde(...)]` on an enum variant.
#[derive(Default)]
pub struct VariantEventAttrs {
    /// Event name for this variant
    pub name: Option<String>,
    /// Serde rename value
    pub rename: Option<String>,
}

/// Generates the BotEvent derive implementation.
pub fn derive_bot_event(input: DeriveInput) -> syn::Result<TokenStream> {
    let attrs = parse_event_attrs(&input.attrs)?;
    let name = &input.ident;

    match &input.data {
        Data::Enum(data) => generate_enum_impl(name, &attrs, &data.variants),
        Data::Struct(_) => generate_struct_impl(name, &attrs),
        Data::Union(_) => Err(syn::Error::new(
            input.span(),
            "BotEvent cannot be derived for unions",
        )),
    }
}

/// Parses `#[event(...)]` attributes from a struct/enum.
fn parse_event_attrs(attrs: &[Attribute]) -> syn::Result<EventAttrs> {
    let mut result = EventAttrs::default();

    for attr in attrs {
        if attr.path().is_ident("event") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("platform") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    result.platform = Some(value.value());
                } else if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    result.name = Some(value.value());
                } else if meta.path.is_ident("parent") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    result.parent = Some(value.value());
                }
                Ok(())
            })?;
        }
    }

    Ok(result)
}

/// Parses `#[event(...)]` and `#[serde(...)]` attributes from a variant.
fn parse_variant_event_attrs(attrs: &[Attribute]) -> syn::Result<VariantEventAttrs> {
    let mut result = VariantEventAttrs::default();

    for attr in attrs {
        if attr.path().is_ident("event") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    result.name = Some(value.value());
                }
                Ok(())
            })?;
        } else if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    result.rename = Some(value.value());
                }
                Ok(())
            })?;
        }
    }

    Ok(result)
}

/// Generates Event and FromEvent implementations for an enum.
fn generate_enum_impl(
    name: &Ident,
    attrs: &EventAttrs,
    variants: &syn::punctuated::Punctuated<Variant, syn::token::Comma>,
) -> syn::Result<TokenStream> {
    let platform = attrs.platform.as_deref().unwrap_or("unknown");
    let platform_lit = syn::LitStr::new(platform, name.span());

    // Generate event_name match arms - delegate to inner type for tuple variants
    let event_name_arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| {
            let variant_name = &v.ident;
            let variant_attrs = parse_variant_event_attrs(&v.attrs).unwrap_or_default();

            // Determine event name: explicit > serde rename > variant name
            let event_suffix = variant_attrs
                .name
                .or(variant_attrs.rename)
                .unwrap_or_else(|| to_snake_case(&variant_name.to_string()));

            let full_name = format!("{platform}.{event_suffix}");
            let full_name_lit = syn::LitStr::new(&full_name, variant_name.span());

            match &v.fields {
                Fields::Unnamed(_) => quote! {
                    #name::#variant_name(inner) => <_ as ::alloy_core::Event>::event_name(inner),
                },
                Fields::Unit => quote! {
                    #name::#variant_name => #full_name_lit,
                },
                Fields::Named(_) => quote! {
                    #name::#variant_name { .. } => #full_name_lit,
                },
            }
        })
        .collect();

    // Generate FromEvent implementation
    let from_event_impl = if let Some(parent) = &attrs.parent {
        let parent_ty: syn::Type = syn::parse_str(parent)
            .unwrap_or_else(|_| panic!("{parent:?} is not a valid type path"));
        quote! {
            impl ::alloy_core::FromEvent for #name {
                fn from_event(root: &dyn ::alloy_core::Event) -> Option<Self> {
                    // First try to parse from raw JSON
                    if let Some(json) = root.raw_json() {
                        if let Ok(event) = ::serde_json::from_str::<#name>(json) {
                            return Some(event);
                        }
                    }
                    // Fallback: try to get from parent
                    if let Some(parent) = <#parent_ty as ::alloy_core::FromEvent>::from_event(root) {
                        return #name::from_parent(&parent);
                    }
                    None
                }
            }

            impl #name {
                /// Tries to extract this event type from a parent event.
                pub fn from_parent(parent: &#parent_ty) -> Option<Self> {
                    let json = ::serde_json::to_string(parent).ok()?;
                    ::serde_json::from_str(&json).ok()
                }
            }
        }
    } else {
        quote! {
            impl ::alloy_core::FromEvent for #name {
                fn from_event(root: &dyn ::alloy_core::Event) -> Option<Self> {
                    // Try raw JSON first
                    if let Some(json) = root.raw_json() {
                        if let Ok(event) = ::serde_json::from_str::<#name>(json) {
                            return Some(event);
                        }
                    }
                    // Fallback: try direct downcast
                    root.as_any().downcast_ref::<#name>().cloned()
                }
            }
        }
    };

    Ok(quote! {
        impl ::alloy_core::Event for #name {
            fn event_name(&self) -> &'static str {
                match self {
                    #(#event_name_arms)*
                }
            }

            fn platform(&self) -> &'static str {
                #platform_lit
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }

        #from_event_impl
    })
}

/// Generates Event and FromEvent implementations for a struct.
fn generate_struct_impl(name: &Ident, attrs: &EventAttrs) -> syn::Result<TokenStream> {
    let platform = attrs.platform.as_deref().unwrap_or("unknown");

    let full_name = attrs
        .name
        .clone()
        .unwrap_or_else(|| format!("{}.{}", platform, to_snake_case(&name.to_string())));
    let full_name_lit = syn::LitStr::new(&full_name, name.span());
    let platform_lit = syn::LitStr::new(platform, name.span());

    // Generate FromEvent implementation
    let from_event_impl = if let Some(parent) = &attrs.parent {
        let parent_ty: syn::Type = syn::parse_str(parent)
            .unwrap_or_else(|_| panic!("{parent:?} is not a valid type path"));
        quote! {
            impl ::alloy_core::FromEvent for #name {
                fn from_event(root: &dyn ::alloy_core::Event) -> Option<Self> {
                    // First try raw JSON
                    if let Some(json) = root.raw_json() {
                        if let Ok(event) = ::serde_json::from_str::<#name>(json) {
                            return Some(event);
                        }
                    }
                    // Try to extract from parent
                    if let Some(parent) = <#parent_ty as ::alloy_core::FromEvent>::from_event(root) {
                        return #name::from_parent(&parent);
                    }
                    None
                }
            }

            impl #name {
                /// Tries to extract this event from a parent event.
                pub fn from_parent(parent: &#parent_ty) -> Option<Self> {
                    let json = ::serde_json::to_string(parent).ok()?;
                    ::serde_json::from_str(&json).ok()
                }
            }
        }
    } else {
        quote! {
            impl ::alloy_core::FromEvent for #name {
                fn from_event(root: &dyn ::alloy_core::Event) -> Option<Self> {
                    // Try raw JSON first
                    if let Some(json) = root.raw_json() {
                        if let Ok(event) = ::serde_json::from_str::<#name>(json) {
                            return Some(event);
                        }
                    }
                    // Fallback: try direct downcast
                    root.as_any().downcast_ref::<#name>().cloned()
                }
            }
        }
    };

    Ok(quote! {
        impl ::alloy_core::Event for #name {
            fn event_name(&self) -> &'static str {
                #full_name_lit
            }

            fn platform(&self) -> &'static str {
                #platform_lit
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }

        #from_event_impl
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Converts CamelCase to snake_case.
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
