use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, FnArg, Ident, ImplItemFn, ItemFn, Pat, ReturnType, Token,
};

/// Parses `#[tool]` or `#[tool(name = "...")]`.
struct ToolAttr {
    name: Option<String>,
}

impl Parse for ToolAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(ToolAttr { name: None });
        }
        let ident: Ident = input.parse()?;
        if ident == "name" {
            input.parse::<Token![=]>()?;
            let lit: syn::LitStr = input.parse()?;
            Ok(ToolAttr {
                name: Some(lit.value()),
            })
        } else {
            Err(syn::Error::new(ident.span(), "expected `name = \"...\"`"))
        }
    }
}

/// Transforms an `async fn` or `impl` block into tool(s).
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let tool_attr = parse_macro_input!(attr as ToolAttr);
    let item_str = item.to_string();
    if item_str.trim().starts_with("impl") {
        match syn::parse::<syn::ItemImpl>(item) {
            Ok(input) => TokenStream::from(expand_impl_block(input)),
            Err(e) => TokenStream::from(e.to_compile_error()),
        }
    } else {
        match syn::parse::<ItemFn>(item) {
            Ok(input) => TokenStream::from(expand_fn(input, tool_attr)),
            Err(e) => TokenStream::from(e.to_compile_error()),
        }
    }
}

// ============ Function variant ============

fn expand_fn(input: ItemFn, tool_attr: ToolAttr) -> TokenStream2 {
    let orig_fn_name = &input.sig.ident;
    let tool_name = tool_attr.name.unwrap_or_else(|| orig_fn_name.to_string());
    let args_struct_name = Ident::new(
        &format!("{}Args", to_pascal_case(&orig_fn_name.to_string())),
        orig_fn_name.span(),
    );

    let params: Vec<_> = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                Some(pt)
            } else {
                None
            }
        })
        .collect();

    let (names, types, param_attrs, docs) = param_data(&params);
    let fn_doc = extract_doc_comment(&input.attrs, orig_fn_name);
    let vis = &input.vis;
    let block = &input.block;

    if input.sig.asyncness.is_none() {
        return quote! { compile_error!("#[tool] requires an async function"); };
    }

    // Validate return type is String
    match &input.sig.output {
        ReturnType::Type(_, ty) => {
            let ty_str = quote!(#ty).to_string();
            if ty_str != "String" {
                return quote! { compile_error!("#[tool] must return String"); };
            }
        }
        ReturnType::Default => {
            return quote! { compile_error!("#[tool] must return String"); };
        }
    }

    let ret_ty = ret_type(&input.sig.output);
    quote! {
        #[derive(serde::Deserialize, schemars::JsonSchema, Clone, Debug)]
        #vis struct #args_struct_name {
            #(
                #(#param_attrs)*
                #[doc = #docs]
                pub #names: #types,
            )*
        }

        impl motif::ToolArgs for #args_struct_name {
            const TOOL_NAME: &'static str = #tool_name;
            const TOOL_DESCRIPTION: &'static str = #fn_doc;
        }

        #vis async fn #orig_fn_name(args: #args_struct_name) -> #ret_ty {
            let #args_struct_name { #(#names),* } = args;
            #block
        }
    }
}

// ============ Impl block variant ============

fn expand_impl_block(input: syn::ItemImpl) -> TokenStream2 {
    let self_ty = &input.self_ty;
    let generics = &input.generics;
    let mut struct_defs = Vec::new();
    let mut new_methods: Vec<TokenStream2> = Vec::new();

    for item in &input.items {
        if let syn::ImplItem::Fn(method) = item {
            let (struct_def, method_impl) = expand_impl_method_parts(method);
            struct_defs.push(struct_def);
            new_methods.push(method_impl);
        } else {
            new_methods.push(item.to_token_stream());
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #(#struct_defs)*
        impl #impl_generics #self_ty #ty_generics #where_clause {
            #(#new_methods)*
        }
    }
}

fn expand_impl_method_parts(method: &ImplItemFn) -> (TokenStream2, TokenStream2) {
    let fn_name = &method.sig.ident;
    let args_struct_name = Ident::new(
        &format!("{}Args", to_pascal_case(&fn_name.to_string())),
        fn_name.span(),
    );

    let params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter(|arg| !matches!(arg, FnArg::Receiver(_)))
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                Some(pt)
            } else {
                None
            }
        })
        .collect();

    let (names, types, param_attrs, docs) = param_data(&params);
    let fn_doc = extract_doc_comment(&method.attrs, fn_name);
    let vis = &method.vis;
    let block = &method.block;
    let tool_name = fn_name.to_string();
    let ret_ty = ret_type(&method.sig.output);
    let has_self = method
        .sig
        .inputs
        .first()
        .map(|a| matches!(a, FnArg::Receiver(_)))
        .unwrap_or(false);

    let struct_def = quote! {
        #[derive(serde::Deserialize, schemars::JsonSchema, Clone, Debug)]
        #vis struct #args_struct_name {
            #(
                #(#param_attrs)*
                #[doc = #docs]
                pub #names: #types,
            )*
        }

        impl motif::ToolArgs for #args_struct_name {
            const TOOL_NAME: &'static str = #tool_name;
            const TOOL_DESCRIPTION: &'static str = #fn_doc;
        }
    };

    let method_body = if has_self {
        quote! {
            #vis async fn #fn_name(self, args: #args_struct_name) -> #ret_ty {
                let #args_struct_name { #(#names),* } = args;
                #block
            }
        }
    } else {
        quote! {
            #vis async fn #fn_name(args: #args_struct_name) -> #ret_ty {
                let #args_struct_name { #(#names),* } = args;
                #block
            }
        }
    };

    (struct_def, method_body)
}

// ============ helpers ============

fn param_data<'a>(
    params: &[&'a syn::PatType],
) -> (
    Vec<&'a Ident>,
    Vec<&'a syn::Type>,
    Vec<Vec<TokenStream2>>,
    Vec<String>,
) {
    let mut names = Vec::new();
    let mut types = Vec::new();
    let mut attrs_tokens = Vec::new();
    let mut docs = Vec::new();
    for pt in params {
        if let Pat::Ident(pi) = pt.pat.as_ref() {
            names.push(&pi.ident);
            types.push(pt.ty.as_ref());
            let non_doc: Vec<_> = pt
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("doc"))
                .map(|a| a.to_token_stream())
                .collect();
            attrs_tokens.push(non_doc);
            docs.push(extract_doc_comment(&pt.attrs, &pi.ident));
        }
    }
    (names, types, attrs_tokens, docs)
}

fn ret_type(output: &ReturnType) -> TokenStream2 {
    match output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    }
}

fn extract_doc_comment(attrs: &[syn::Attribute], fallback: &Ident) -> String {
    let mut doc = String::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(mnv) = &attr.meta {
                if let syn::Expr::Lit(lit) = &mnv.value {
                    if let syn::Lit::Str(s) = &lit.lit {
                        if !doc.is_empty() {
                            doc.push('\n');
                        }
                        doc.push_str(s.value().trim());
                    }
                }
            }
        }
    }
    if doc.is_empty() {
        fallback.to_string()
    } else {
        doc
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut r = String::new();
    let mut cap = true;
    for ch in s.chars() {
        if ch == '_' {
            cap = true;
        } else if cap {
            r.push(ch.to_ascii_uppercase());
            cap = false;
        } else {
            r.push(ch);
        }
    }
    r
}
