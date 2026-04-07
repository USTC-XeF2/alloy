use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{Attribute, Fields, Ident, ItemEnum, ItemStruct, Meta, Path, Type, Variant};

pub fn expand_event_root(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let root_attr = parse_event_root_attr(attr)?;
    let mut root_struct: ItemStruct = syn::parse2(item)?;

    let (data_field_ident, data_field_ty) = find_root_data_field(&root_struct)?;

    strip_custom_attrs(&mut root_struct.attrs);
    if let Fields::Named(fields) = &mut root_struct.fields {
        for field in &mut fields.named {
            strip_custom_attrs(&mut field.attrs);
        }
    }

    let root_name = &root_struct.ident;
    let platform = root_attr.platform;

    Ok(quote! {
        #root_struct

        impl ::alloy_core::EventRoot for #root_name {
            fn platform(&self) -> &'static str {
                #platform
            }

            fn event_id(&self) -> String {
                let mut s = String::new();
                self.#data_field_ident.write_id(&mut s);
                s
            }

            fn event_type(&self) -> ::alloy_core::EventType {
                self.#data_field_ident.event_type()
            }

            fn scene(&self) -> Option<::alloy_core::Scene> {
                self.#data_field_ident.scene()
            }

            fn plain_text(&self) -> String {
                self.#data_field_ident.plain_text()
            }

            fn rich_text(&self) -> Vec<::alloy_core::RichTextSegment> {
                self.#data_field_ident.rich_text()
            }
        }

        impl #root_name {
            fn data(&self) -> #data_field_ty {
                self.#data_field_ident.clone()
            }
        }
    })
}

pub fn expand_event_data(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let data_attr = parse_event_data_attr(attr)?;
    let mut data_enum: ItemEnum = syn::parse2(item)?;

    let enum_name = data_enum.ident.clone();
    let variant_specs = collect_variant_specs(&enum_name, &data_enum.variants)?;

    strip_custom_attrs(&mut data_enum.attrs);
    for variant in &mut data_enum.variants {
        strip_custom_attrs(&mut variant.attrs);
        if let Fields::Named(fields) = &mut variant.fields {
            for field in &mut fields.named {
                strip_custom_attrs(&mut field.attrs);
            }
        }
    }

    let write_id_arms = variant_specs.iter().map(write_id_arm);
    let event_type_arms = variant_specs.iter().map(event_type_arm);
    let scene_arms = variant_specs
        .iter()
        .map(scene_arm)
        .collect::<syn::Result<Vec<_>>>()?;
    let (plain_text_arms, rich_text_arms): (Vec<_>, Vec<_>) =
        variant_specs.iter().map(message_text_arms).unzip();

    let view_defs = variant_specs
        .iter()
        .map(|spec| generate_view_tokens(&enum_name, &data_attr.parent, spec));

    Ok(quote! {
        #data_enum

        impl #enum_name {
            fn write_id(&self, s: &mut String) {
                match self {
                    #(#write_id_arms)*
                }
            }

            fn event_type(&self) -> ::alloy_core::EventType {
                match self {
                    #(#event_type_arms)*
                }
            }

            fn scene(&self) -> Option<::alloy_core::Scene> {
                match self {
                    #(#scene_arms)*
                }
            }

            fn plain_text(&self) -> String {
                match self {
                    #(#plain_text_arms)*
                }
            }

            fn rich_text(&self) -> Vec<::alloy_core::RichTextSegment> {
                match self {
                    #(#rich_text_arms)*
                }
            }
        }

        #(#view_defs)*
    })
}

struct EventRootAttr {
    platform: syn::LitStr,
}

struct EventDataAttr {
    parent: Type,
}

struct EventViewAttr {
    name: Ident,
    id: syn::LitStr,
    event_type: Option<Ident>,
    scene: Option<Ident>,
    scene_func: Option<Path>,
}

#[derive(Clone)]
struct VariantFieldSpec {
    ident: Ident,
    ty: Type,
    is_user_id: bool,
    is_group_id: bool,
    is_guild_id: bool,
    is_message: bool,
    is_event_data: bool,
    cfg_attrs: Vec<Attribute>,
}

struct VariantSpec {
    variant_ident: Ident,
    view_attr: EventViewAttr,
    fields: Vec<VariantFieldSpec>,
    cfg_attrs: Vec<Attribute>,
}

fn collect_cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .cloned()
        .collect()
}

fn arm_with_cfg(spec: &VariantSpec, arm: TokenStream) -> TokenStream {
    let cfg_attrs = &spec.cfg_attrs;
    quote! {
        #(#cfg_attrs)*
        #arm
    }
}

fn parse_event_root_attr(attr: TokenStream) -> syn::Result<EventRootAttr> {
    let mut platform: Option<syn::LitStr> = None;

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("platform") {
            platform = Some(meta.value()?.parse::<syn::LitStr>()?);
            return Ok(());
        }
        Err(meta.error("unsupported key in #[event_root(...)], expected `platform`"))
    });

    parser.parse2(attr)?;

    let platform = platform.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[event_root(...)] requires `platform = \"...\"`",
        )
    })?;

    Ok(EventRootAttr { platform })
}

fn parse_event_data_attr(attr: TokenStream) -> syn::Result<EventDataAttr> {
    let mut parent: Option<Type> = None;

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("parent") {
            parent = Some(meta.value()?.parse::<Type>()?);
            return Ok(());
        }
        Err(meta.error("unsupported key in #[event_data(...)], expected `parent`"))
    });

    parser.parse2(attr)?;

    let parent = parent.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[event_data(...)] requires `parent = Type`",
        )
    })?;

    Ok(EventDataAttr { parent })
}

fn parse_event_view_attr(attrs: &[Attribute], variant: &Variant) -> syn::Result<EventViewAttr> {
    let attr = attrs
        .iter()
        .find(|a| a.path().is_ident("event_view"))
        .ok_or_else(|| {
            syn::Error::new(
                variant.span(),
                "each variant in #[event_data] enum must have #[event_view(name = ..., id = ...)]",
            )
        })?;

    let mut name: Option<Ident> = None;
    let mut id: Option<syn::LitStr> = None;
    let mut event_type: Option<Ident> = None;
    let mut scene: Option<Ident> = None;
    let mut scene_func: Option<Path> = None;

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("name") {
            name = Some(meta.value()?.parse::<Ident>()?);
            return Ok(());
        }
        if meta.path.is_ident("id") {
            id = Some(meta.value()?.parse::<syn::LitStr>()?);
            return Ok(());
        }
        if meta.path.is_ident("type") {
            event_type = Some(meta.value()?.parse::<Ident>()?);
            return Ok(());
        }
        if meta.path.is_ident("scene") {
            scene = Some(meta.value()?.parse::<Ident>()?);
            return Ok(());
        }
        if meta.path.is_ident("scene_func") {
            scene_func = Some(meta.value()?.parse::<Path>()?);
            return Ok(());
        }
        Err(meta
            .error("unsupported key in #[event_view(...)], expected name/id/type/scene/scene_func"))
    })?;

    let name =
        name.ok_or_else(|| syn::Error::new(variant.span(), "#[event_view] missing `name = ...`"))?;
    let id =
        id.ok_or_else(|| syn::Error::new(variant.span(), "#[event_view] missing `id = \"...\"`"))?;

    Ok(EventViewAttr {
        name,
        id,
        event_type,
        scene,
        scene_func,
    })
}

fn collect_variant_specs(
    enum_name: &Ident,
    variants: &syn::punctuated::Punctuated<Variant, syn::token::Comma>,
) -> syn::Result<Vec<VariantSpec>> {
    let mut specs = Vec::new();

    for variant in variants {
        let view_attr = parse_event_view_attr(&variant.attrs, variant)?;
        let variant_cfg_attrs = collect_cfg_attrs(&variant.attrs);

        let mut fields = Vec::new();
        match &variant.fields {
            Fields::Unit => {}
            Fields::Named(named) => {
                for field in &named.named {
                    let ident = field
                        .ident
                        .clone()
                        .ok_or_else(|| syn::Error::new(field.span(), "named field expected"))?;
                    let mut is_user_id = false;
                    let mut is_group_id = false;
                    let mut is_guild_id = false;
                    let mut is_message = false;
                    let mut is_event_data = false;
                    let cfg_attrs = collect_cfg_attrs(&field.attrs);

                    for attr in &field.attrs {
                        if attr.path().is_ident("event_field") {
                            attr.parse_nested_meta(|meta| {
                                if meta.path.is_ident("user_id") {
                                    is_user_id = true;
                                    return Ok(());
                                }
                                if meta.path.is_ident("message") {
                                    is_message = true;
                                    return Ok(());
                                }
                                if meta.path.is_ident("group_id") {
                                    is_group_id = true;
                                    return Ok(());
                                }
                                if meta.path.is_ident("guild_id") {
                                    is_guild_id = true;
                                    return Ok(());
                                }
                                Err(meta.error(
                                    "unsupported key in #[event_field(...)], expected user_id/group_id/guild_id/message",
                                ))
                            })?;
                        }
                        if attr.path().is_ident("event_data") {
                            is_event_data = true;
                            if !matches!(&attr.meta, Meta::Path(_)) {
                                return Err(syn::Error::new(
                                    attr.span(),
                                    "field #[event_data] does not accept arguments, use #[event_data]",
                                ));
                            }
                        }
                    }

                    fields.push(VariantFieldSpec {
                        ident,
                        ty: field.ty.clone(),
                        is_user_id,
                        is_group_id,
                        is_guild_id,
                        is_message,
                        is_event_data,
                        cfg_attrs,
                    });
                }
            }
            Fields::Unnamed(_) => {
                return Err(syn::Error::new(
                    variant.span(),
                    format!(
                        "{}::{} does not support tuple variants, use named or unit variant",
                        enum_name, variant.ident
                    ),
                ));
            }
        }

        let nested_count = fields.iter().filter(|f| f.is_event_data).count();
        if nested_count > 1 {
            return Err(syn::Error::new(
                variant.span(),
                "a variant can contain at most one #[event_data] field",
            ));
        }

        specs.push(VariantSpec {
            variant_ident: variant.ident.clone(),
            view_attr,
            fields,
            cfg_attrs: variant_cfg_attrs,
        });
    }

    Ok(specs)
}

fn nested_data_field(spec: &VariantSpec) -> Option<&VariantFieldSpec> {
    spec.fields.iter().find(|f| f.is_event_data)
}

fn user_id_field(spec: &VariantSpec) -> Option<&VariantFieldSpec> {
    spec.fields.iter().find(|f| f.is_user_id)
}

fn group_id_field(spec: &VariantSpec) -> Option<&VariantFieldSpec> {
    spec.fields.iter().find(|f| f.is_group_id)
}

fn guild_id_field(spec: &VariantSpec) -> Option<&VariantFieldSpec> {
    spec.fields.iter().find(|f| f.is_guild_id)
}

fn message_field(spec: &VariantSpec) -> Option<&VariantFieldSpec> {
    spec.fields.iter().find(|f| f.is_message)
}

fn scene_arm(spec: &VariantSpec) -> syn::Result<TokenStream> {
    let variant_ident = &spec.variant_ident;

    if let Some(scene_func) = &spec.view_attr.scene_func {
        if spec.fields.is_empty() {
            return Ok(arm_with_cfg(
                spec,
                quote! {
                    Self::#variant_ident => #scene_func(self),
                },
            ));
        }

        return Ok(arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident { .. } => #scene_func(self),
            },
        ));
    }

    if let Some(view_scene) = &spec.view_attr.scene {
        let scene = view_scene.to_string();
        return match scene.as_str() {
            "Private" => {
                let user_id = user_id_field(spec).ok_or_else(|| {
                    syn::Error::new(
                        view_scene.span(),
                        format!(
                            "variant {variant_ident} declares scene = Private but is missing #[event_field(user_id)]"
                        ),
                    )
                })?;
                let user_ident = &user_id.ident;
                Ok(arm_with_cfg(
                    spec,
                    quote! {
                        Self::#variant_ident { #user_ident, .. } => Some(::alloy_core::Scene::Private {
                            user_id: #user_ident.to_string(),
                        }),
                    },
                ))
            }
            "Group" => {
                let group_id = group_id_field(spec).ok_or_else(|| {
                    syn::Error::new(
                        view_scene.span(),
                        format!(
                            "variant {variant_ident} declares scene = Group but is missing #[event_field(group_id)]"
                        ),
                    )
                })?;
                let group_ident = &group_id.ident;
                if let Some(user_id) = user_id_field(spec) {
                    let user_ident = &user_id.ident;
                    Ok(arm_with_cfg(
                        spec,
                        quote! {
                            Self::#variant_ident { #group_ident, #user_ident, .. } => Some(::alloy_core::Scene::Group {
                                group_id: #group_ident.to_string(),
                                user_id: Some(#user_ident.to_string()),
                            }),
                        },
                    ))
                } else {
                    Ok(arm_with_cfg(
                        spec,
                        quote! {
                            Self::#variant_ident { #group_ident, .. } => Some(::alloy_core::Scene::Group {
                                group_id: #group_ident.to_string(),
                                user_id: None,
                            }),
                        },
                    ))
                }
            }
            "Guild" => {
                let guild_id = guild_id_field(spec).ok_or_else(|| {
                    syn::Error::new(
                        view_scene.span(),
                        format!(
                            "variant {variant_ident} declares scene = Guild but is missing #[event_field(guild_id)]"
                        ),
                    )
                })?;
                let guild_ident = &guild_id.ident;
                Ok(arm_with_cfg(
                    spec,
                    quote! {
                        Self::#variant_ident { #guild_ident, .. } => Some(::alloy_core::Scene::Guild {
                            guild_id: #guild_ident.to_string(),
                        }),
                    },
                ))
            }
            _ => Err(syn::Error::new(
                view_scene.span(),
                "unsupported scene in #[event_view(...)] , expected Private/Group/Guild",
            )),
        };
    }

    if spec.fields.is_empty() {
        return Ok(arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident => None,
            },
        ));
    }

    if let Some(nested) = nested_data_field(spec) {
        let nested_ident = &nested.ident;
        return Ok(arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident { #nested_ident, .. } => #nested_ident.scene(),
            },
        ));
    }

    Ok(arm_with_cfg(
        spec,
        quote! {
            Self::#variant_ident { .. } => None,
        },
    ))
}

fn write_id_arm(spec: &VariantSpec) -> TokenStream {
    let variant_ident = &spec.variant_ident;
    let view_id = &spec.view_attr.id;

    if spec.fields.is_empty() {
        return arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident => s.push_str(#view_id),
            },
        );
    }

    if let Some(nested) = nested_data_field(spec) {
        let nested_ident = &nested.ident;
        let id_with_dot = syn::LitStr::new(&(view_id.value() + "."), view_id.span());
        return arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident { #nested_ident, .. } => {
                    s.push_str(#id_with_dot);
                    #nested_ident.write_id(s);
                }
            },
        );
    }

    arm_with_cfg(
        spec,
        quote! {
            Self::#variant_ident { .. } => s.push_str(#view_id),
        },
    )
}

fn event_type_arm(spec: &VariantSpec) -> TokenStream {
    let variant_ident = &spec.variant_ident;

    if spec.fields.is_empty() {
        if let Some(event_type) = &spec.view_attr.event_type {
            return arm_with_cfg(
                spec,
                quote! {
                    Self::#variant_ident => ::alloy_core::EventType::#event_type,
                },
            );
        }
        return arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident => ::alloy_core::EventType::Other,
            },
        );
    }

    if let Some(event_type) = &spec.view_attr.event_type {
        return arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident { .. } => ::alloy_core::EventType::#event_type,
            },
        );
    }

    if let Some(nested) = nested_data_field(spec) {
        let nested_ident = &nested.ident;
        return arm_with_cfg(
            spec,
            quote! {
                Self::#variant_ident { #nested_ident, .. } => #nested_ident.event_type(),
            },
        );
    }

    arm_with_cfg(
        spec,
        quote! {
            Self::#variant_ident { .. } => ::alloy_core::EventType::Other,
        },
    )
}

fn message_text_arms(spec: &VariantSpec) -> (TokenStream, TokenStream) {
    let variant_ident = &spec.variant_ident;

    if spec.fields.is_empty() {
        return (
            arm_with_cfg(spec, quote! { Self::#variant_ident => String::new(), }),
            arm_with_cfg(spec, quote! { Self::#variant_ident => Vec::new(), }),
        );
    }

    if let Some(message) = message_field(spec) {
        let message_ident = &message.ident;
        return (
            arm_with_cfg(
                spec,
                quote! { Self::#variant_ident { #message_ident, .. } => #message_ident.to_string(), },
            ),
            arm_with_cfg(
                spec,
                quote! { Self::#variant_ident { #message_ident, .. } => #message_ident.extract_rich_text(), },
            ),
        );
    }

    if let Some(nested) = nested_data_field(spec) {
        let nested_ident = &nested.ident;
        return (
            arm_with_cfg(
                spec,
                quote! { Self::#variant_ident { #nested_ident, .. } => #nested_ident.plain_text(), },
            ),
            arm_with_cfg(
                spec,
                quote! { Self::#variant_ident { #nested_ident, .. } => #nested_ident.rich_text(), },
            ),
        );
    }

    (
        arm_with_cfg(
            spec,
            quote! { Self::#variant_ident { .. } => String::new(), },
        ),
        arm_with_cfg(spec, quote! { Self::#variant_ident { .. } => Vec::new(), }),
    )
}

fn generate_view_tokens(enum_name: &Ident, parent_ty: &Type, spec: &VariantSpec) -> TokenStream {
    let view_name = &spec.view_attr.name;
    let variant_ident = &spec.variant_ident;
    let variant_cfg_attrs = &spec.cfg_attrs;

    let view_fields = spec.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let cfg_attrs = &field.cfg_attrs;
        quote! {
            #(#cfg_attrs)*
            pub #ident: #ty
        }
    });

    let field_patterns = spec.fields.iter().map(|field| {
        let ident = &field.ident;
        let cfg_attrs = &field.cfg_attrs;
        quote! {
            #(#cfg_attrs)*
            #ident
        }
    });

    let field_inits = spec.fields.iter().map(|field| {
        let ident = &field.ident;
        let cfg_attrs = &field.cfg_attrs;
        quote! {
            #(#cfg_attrs)*
            #ident
        }
    });

    let from_root = if spec.fields.is_empty() {
        quote! {
            fn from_root(event: Self::Root) -> Option<Self> {
                if let Some(parent) = Self::Parent::from_root(event)
                    && let #enum_name::#variant_ident = parent.data()
                {
                    Some(Self { parent })
                } else {
                    None
                }
            }
        }
    } else {
        quote! {
            fn from_root(event: Self::Root) -> Option<Self> {
                if let Some(parent) = Self::Parent::from_root(event)
                    && let #enum_name::#variant_ident { #(#field_patterns,)* .. } = parent.data()
                {
                    Some(Self {
                        parent,
                        #(#field_inits,)*
                    })
                } else {
                    None
                }
            }
        }
    };

    let nested_data_impl = if let Some(nested) = nested_data_field(spec) {
        let nested_ident = &nested.ident;
        let nested_ty = &nested.ty;
        let nested_cfg_attrs = &nested.cfg_attrs;
        quote! {
            #(#nested_cfg_attrs)*
            impl #view_name {
                fn data(&self) -> #nested_ty {
                    self.#nested_ident.clone()
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #(#variant_cfg_attrs)*
        #[derive(Debug, Clone)]
        pub struct #view_name {
            parent: #parent_ty,
            #(#view_fields,)*
        }

        #(#variant_cfg_attrs)*
        impl ::alloy_core::EventView for #view_name {
            type Root = <#parent_ty as ::alloy_core::EventView>::Root;
            type Parent = #parent_ty;

            #from_root

            fn root(&self) -> &Self::Root {
                self.parent.root()
            }
        }

        #nested_data_impl

        #(#variant_cfg_attrs)*
        impl ::std::ops::Deref for #view_name {
            type Target = #parent_ty;

            fn deref(&self) -> &Self::Target {
                &self.parent
            }
        }
    }
}

fn find_root_data_field(root_struct: &ItemStruct) -> syn::Result<(Ident, Type)> {
    let Fields::Named(fields) = &root_struct.fields else {
        return Err(syn::Error::new(
            root_struct.span(),
            "#[event_root] only supports structs with named fields",
        ));
    };

    let mut found: Option<(Ident, Type)> = None;
    for field in &fields.named {
        let mut has_event_data = false;
        for attr in &field.attrs {
            if !attr.path().is_ident("event_data") {
                continue;
            }
            if !matches!(&attr.meta, Meta::Path(_)) {
                return Err(syn::Error::new(
                    attr.span(),
                    "field #[event_data] does not accept arguments, use #[event_data]",
                ));
            }
            has_event_data = true;
        }
        if !has_event_data {
            continue;
        }

        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new(field.span(), "named field expected"))?;

        if found.is_some() {
            return Err(syn::Error::new(
                field.span(),
                "#[event_root] expects exactly one field with #[event_data(...)]",
            ));
        }

        found = Some((ident, field.ty.clone()));
    }

    found.ok_or_else(|| {
        syn::Error::new(
            root_struct.span(),
            "#[event_root] requires one field marked with #[event_data(...)]",
        )
    })
}

fn strip_custom_attrs(attrs: &mut Vec<Attribute>) {
    attrs.retain(|attr| {
        let path = attr.path();
        !(path.is_ident("event_root")
            || path.is_ident("event_data")
            || path.is_ident("event_field")
            || path.is_ident("event_view"))
    });
}
