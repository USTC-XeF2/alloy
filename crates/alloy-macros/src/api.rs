use heck::ToSnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{
    Error, Expr, ExprLit, Field, Fields, Ident, ItemStruct, Lit, LitBool, Path, Result, Token,
    Type, parse_quote,
};

pub fn expand_api_payload(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args = ApiPayloadArgs::parse(attr)?;
    let mut input: ItemStruct = syn::parse2(item)?;

    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "api_payload does not support generic structs",
        ));
    }

    let fields = match &mut input.fields {
        Fields::Named(named) => &mut named.named,
        _ => {
            return Err(Error::new_spanned(
                &input,
                "api_payload only supports structs with named fields",
            ));
        }
    };

    let mut field_infos = Vec::new();
    for field in fields.iter_mut() {
        field_infos.push(FieldInfo::from_field(field)?);
    }

    let struct_ident = input.ident.clone();
    let struct_doc_attrs = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect::<Vec<_>>();
    let action_ident = Ident::new(&struct_ident.to_string().to_snake_case(), Span::call_site());
    let builder_trait_ident = format_ident!("{}Builder", struct_ident);
    let response_ty = args.response;
    let bot_ty = args.bot;
    let response_wrapper_ident = format_ident!("{}Response", struct_ident);
    let has_response_field = args.field.is_some();
    let response_wrapper_tokens = if let Some(field_key) = &args.field {
        let vis = &input.vis;
        quote! {
            #[derive(Debug, ::serde::Deserialize)]
            #vis struct #response_wrapper_ident {
                #[serde(rename = #field_key)]
                inner: #response_ty,
            }

            impl ::core::ops::Deref for #response_wrapper_ident {
                type Target = #response_ty;

                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl ::core::ops::DerefMut for #response_wrapper_ident {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.inner
                }
            }

            impl ::core::convert::AsRef<#response_ty> for #response_wrapper_ident {
                fn as_ref(&self) -> &#response_ty {
                    &self.inner
                }
            }

            impl ::core::convert::AsMut<#response_ty> for #response_wrapper_ident {
                fn as_mut(&mut self) -> &mut #response_ty {
                    &mut self.inner
                }
            }

            impl ::core::convert::From<#response_wrapper_ident> for #response_ty {
                fn from(wrapper: #response_wrapper_ident) -> Self {
                    wrapper.inner
                }
            }
        }
    } else {
        quote! {}
    };
    let api_payload_response_ty = if has_response_field {
        quote!(#response_wrapper_ident)
    } else {
        quote!(#response_ty)
    };

    let required_fields: Vec<&FieldInfo> = field_infos
        .iter()
        .filter(|f| !f.default.is_enabled())
        .collect();
    let default_fields: Vec<&FieldInfo> = field_infos
        .iter()
        .filter(|f| f.default.is_enabled())
        .collect();

    let required_sig = required_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let cfg = &f.cfg_attrs;
            if f.into {
                quote!(#(#cfg)* #ident: impl ::core::convert::Into<#ty>)
            } else {
                quote!(#(#cfg)* #ident: #ty)
            }
        })
        .collect::<Vec<_>>();

    let default_builder_trait_methods = default_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let cfg = &f.cfg_attrs;
            if f.into {
                quote!(#(#cfg)* fn #ident(self, #ident: impl ::core::convert::Into<#ty>) -> Self;)
            } else {
                quote!(#(#cfg)* fn #ident(self, #ident: #ty) -> Self;)
            }
        })
        .collect::<Vec<_>>();

    let default_builder_impl_methods = default_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let cfg = &f.cfg_attrs;
            if f.into {
                quote! {
                    #(#cfg)*
                    fn #ident(mut self, #ident: impl ::core::convert::Into<#ty>) -> Self {
                        self.payload_mut().#ident = #ident.into();
                        self
                    }
                }
            } else {
                quote! {
                    #(#cfg)*
                    fn #ident(mut self, #ident: #ty) -> Self {
                        self.payload_mut().#ident = #ident;
                        self
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let init_fields = field_infos
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let cfg = &f.cfg_attrs;
            match &f.default {
                FieldDefault::None => {
                    if f.into {
                        quote!(#(#cfg)* #ident: #ident.into())
                    } else {
                        quote!(#(#cfg)* #ident)
                    }
                }
                FieldDefault::Trait => quote!(#(#cfg)* #ident: ::core::default::Default::default()),
                FieldDefault::Function(path) => quote!(#(#cfg)* #ident: #path()),
            }
        })
        .collect::<Vec<_>>();

    let builder_tokens = if default_fields.is_empty() {
        quote! {}
    } else {
        quote! {
            pub trait #builder_trait_ident {
                #(#default_builder_trait_methods)*
            }

            impl #builder_trait_ident for ::alloy_core::bot::ApiRequest<'_, #struct_ident> {
                #(#default_builder_impl_methods)*
            }
        }
    };

    let ext_tokens = if args.ext {
        quote! {
            impl #bot_ty {
                #(#struct_doc_attrs)*
                pub fn #action_ident(&self, #(#required_sig),*) -> ::alloy_core::bot::ApiRequest<'_, #struct_ident> {
                    ::alloy_core::bot::ApiRequest::new(
                        self,
                        #struct_ident {
                            #(#init_fields),*
                        },
                    )
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #input

        #response_wrapper_tokens

        impl ::alloy_core::bot::ApiPayload for #struct_ident {
            const NAME: &'static str = stringify!(#action_ident);

            type Client = #bot_ty;
            type Response = #api_payload_response_ty;
        }

        #builder_tokens

        #ext_tokens
    })
}

struct ApiPayloadArgs {
    bot: Type,
    response: Type,
    ext: bool,
    field: Option<syn::LitStr>,
}

impl ApiPayloadArgs {
    fn parse(attr: TokenStream) -> Result<Self> {
        let mut bot = None;
        let mut response = None;
        let mut ext = None;
        let mut field = None;

        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("bot") {
                bot = Some(meta.value()?.parse::<Type>()?);
                return Ok(());
            }

            if meta.path.is_ident("response") {
                response = Some(meta.value()?.parse::<Type>()?);
                return Ok(());
            }

            if meta.path.is_ident("ext") {
                ext = Some(meta.value()?.parse::<LitBool>()?.value);
                return Ok(());
            }

            if meta.path.is_ident("field") {
                field = Some(meta.value()?.parse::<syn::LitStr>()?);
                return Ok(());
            }

            Err(meta.error("unsupported api_payload option"))
        });

        parser.parse2(attr)?;

        let bot = bot.ok_or_else(|| Error::new(Span::call_site(), "missing `bot = ...`"))?;
        let response = response.unwrap_or_else(|| parse_quote!(()));

        Ok(Self {
            bot,
            response,
            ext: ext.unwrap_or(true),
            field,
        })
    }
}

struct FieldInfo {
    ident: Ident,
    ty: Type,
    default: FieldDefault,
    into: bool,
    cfg_attrs: Vec<syn::Attribute>,
}

enum FieldDefault {
    None,
    Trait,
    Function(Path),
}

impl FieldDefault {
    fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl FieldInfo {
    fn from_field(field: &mut Field) -> Result<Self> {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| Error::new(field.span(), "expected named field"))?;

        let mut default = FieldDefault::None;
        let mut into = false;
        let mut cfg_attrs = Vec::new();

        let mut kept_attrs = Vec::with_capacity(field.attrs.len());
        for attr in std::mem::take(&mut field.attrs) {
            if attr.path().is_ident("cfg") {
                cfg_attrs.push(attr.clone());
                kept_attrs.push(attr);
            } else if attr.path().is_ident("api_param") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("default") {
                        if meta.input.peek(Token![=]) {
                            let expr = meta.value()?.parse::<Expr>()?;
                            match expr {
                                Expr::Lit(ExprLit {
                                    lit: Lit::Str(lit), ..
                                }) => {
                                    let func: Path = lit.parse()?;
                                    default = FieldDefault::Function(func);
                                }
                                Expr::Lit(ExprLit {
                                    lit: Lit::Bool(lit),
                                    ..
                                }) => {
                                    default = if lit.value {
                                        FieldDefault::Trait
                                    } else {
                                        FieldDefault::None
                                    };
                                }
                                _ => {
                                    return Err(meta
                                        .error("`default` expects bool or string function path"));
                                }
                            }
                        } else {
                            default = FieldDefault::Trait;
                        }
                        return Ok(());
                    }

                    if meta.path.is_ident("into") {
                        if meta.input.peek(Token![=]) {
                            into = meta.value()?.parse::<LitBool>()?.value;
                        } else {
                            into = true;
                        }
                        return Ok(());
                    }

                    Err(meta.error("unsupported api_param option"))
                })?;
            } else {
                kept_attrs.push(attr);
            }
        }

        field.attrs = kept_attrs;

        Ok(Self {
            ident,
            ty: field.ty.clone(),
            default,
            into,
            cfg_attrs,
        })
    }
}
