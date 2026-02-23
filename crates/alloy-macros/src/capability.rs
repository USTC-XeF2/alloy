use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Ident, ItemFn, parse_macro_input};

/// Implementation of `#[register_capability(cap_type)]` attribute macro.
///
/// Leaves the decorated `async fn` unchanged and appends a
/// `#[::alloy_core::linkme::distributed_slice]` static that wires the function
/// into the corresponding capability registry in `alloy-core`.
pub fn register_capability(attr: TokenStream, item: TokenStream) -> TokenStream {
    let cap_type = parse_macro_input!(attr as Ident);
    let func = parse_macro_input!(item as ItemFn);

    let fn_name = &func.sig.ident;
    let fn_name_upper = fn_name.to_string().to_uppercase();
    let static_name = Ident::new(
        &format!("_CAPABILITY_REGISTER_{fn_name_upper}"),
        Span::call_site(),
    );

    let cap_str = cap_type.to_string();

    let (slice, fn_ty, arg_decls, arg_names) = match cap_str.as_str() {
        "ws_client" => (
            quote!(::alloy_core::WS_CONNECT_REGISTRY),
            quote!(::alloy_core::WsConnectFn),
            quote!(
                config: ::alloy_core::WsClientConfig,
                handler: ::std::sync::Arc<dyn ::alloy_core::ConnectionHandler>
            ),
            quote!(config, handler),
        ),
        "ws_server" => (
            quote!(::alloy_core::WS_LISTEN_REGISTRY),
            quote!(::alloy_core::WsListenFn),
            quote!(
                addr: ::std::string::String,
                path: ::std::string::String,
                handler: ::std::sync::Arc<dyn ::alloy_core::ConnectionHandler>
            ),
            quote!(addr, path, handler),
        ),
        "http_client" => (
            quote!(::alloy_core::HTTP_START_CLIENT_REGISTRY),
            quote!(::alloy_core::HttpStartClientFn),
            quote!(
                bot_id: ::std::string::String,
                config: ::alloy_core::HttpClientConfig,
                handler: ::std::sync::Arc<dyn ::alloy_core::ConnectionHandler>
            ),
            quote!(bot_id, config, handler),
        ),
        "http_server" => (
            quote!(::alloy_core::HTTP_LISTEN_REGISTRY),
            quote!(::alloy_core::HttpListenFn),
            quote!(
                addr: ::std::string::String,
                path: ::std::string::String,
                handler: ::std::sync::Arc<dyn ::alloy_core::ConnectionHandler>
            ),
            quote!(addr, path, handler),
        ),
        other => {
            return syn::Error::new(
                cap_type.span(),
                format!(
                    "unknown capability type `{other}`, \
                     expected one of: ws_client, ws_server, http_client, http_server"
                ),
            )
            .into_compile_error()
            .into();
        }
    };

    quote! {
        #func

        #[::alloy_core::linkme::distributed_slice(#slice)]
        #[linkme(crate = ::alloy_core::linkme)]
        static #static_name: #fn_ty =
            |#arg_decls| ::futures::FutureExt::boxed(#fn_name(#arg_names));
    }
    .into()
}
