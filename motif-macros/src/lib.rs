use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Pat, ReturnType, Type};

/// Transforms an async function into a tool with auto-generated args struct.
///
/// ## Usage
///
/// ```ignore
/// use motif::tool;
///
/// /// Search the web
/// #[tool]
/// async fn web_search(
///     /// Search query
///     query: String,
/// ) -> String {
///     format!("Results for: {}", query)
/// }
///
/// // Expands to:
/// // #[derive(serde::Deserialize, schemars::JsonSchema, Clone, Debug)]
/// // pub struct WebSearchArgs { pub query: String }
/// // impl motif::ToolArgs for WebSearchArgs { ... }
/// // async fn web_search(args: WebSearchArgs) -> String { ... }
/// ```
#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let args_struct_name =
        syn::Ident::new(&format!("{}Args", to_pascal_case(&fn_name.to_string())), fn_name.span());

    // Extract parameters (skip self in methods)
    let params: Vec<_> = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                    let name = &pat_ident.ident;
                    let ty = &pat_type.ty;
                    let doc = extract_doc_comment(&pat_type.attrs);
                    return Some((name.clone(), ty.clone(), doc));
                }
            }
            None
        })
        .collect();

    let param_names: Vec<_> = params.iter().map(|(n, _, _)| n).collect();
    let param_types: Vec<_> = params.iter().map(|(_, t, _)| t).collect();
    let param_docs: Vec<_> = params.iter().map(|(_, _, d)| d).collect();

    // Collect fn-level doc comments for tool description
    let fn_doc = extract_doc_comment(&input.attrs);

    let visibility = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let is_async = sig.asyncness.is_some();
    let ret_ty = match &sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    // The original function body references parameter names directly.
    // After transformation, params come from the args struct: `let ArgName { p1, p2 } = args;`
    let destruct = quote! {
        let #args_struct_name { #(#param_names),* } = args;
    };

    let expanded = if is_async {
        quote! {
            #[derive(serde::Deserialize, schemars::JsonSchema, Clone, Debug)]
            #visibility struct #args_struct_name {
                #(
                    #[doc = #param_docs]
                    pub #param_names: #param_types,
                )*
            }

            impl motif::ToolArgs for #args_struct_name {
                const TOOL_NAME: &'static str = stringify!(#fn_name);
                const TOOL_DESCRIPTION: &'static str = #fn_doc;
            }

            #visibility async fn #fn_name(args: #args_struct_name) -> #ret_ty {
                #destruct
                #block
            }
        }
    } else {
        quote! {
            compile_error!("#[tool] requires an async function");
        }
    };

    TokenStream::from(expanded)
}

fn extract_doc_comment(attrs: &[syn::Attribute]) -> String {
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
        doc = stringify!(#fn_name).to_string();
    }
    doc
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Same as #[tool] but for methods (used via `motif::tool_impl`).
/// Attributes on method params are preserved.
#[proc_macro_attribute]
pub fn tool_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the entire impl block
    let input_str = item.to_string();
    // For method-level tools, we need a different approach — the macro
    // is applied to individual methods within an impl block.
    // For now, support #[tool] on individual methods by parsing as ItemFn.
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let args_struct_name =
        syn::Ident::new(&format!("{}Args", to_pascal_case(&fn_name.to_string())), fn_name.span());

    let is_method = input.sig.inputs.first().map(|arg| {
        if let FnArg::Receiver(_) = arg { true } else { false }
    }).unwrap_or(false);

    let params: Vec<_> = input.sig.inputs.iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                    let name = &pat_ident.ident;
                    let ty = &pat_type.ty;
                    let doc = extract_doc_comment(&pat_type.attrs);
                    return Some((name.clone(), ty.clone(), doc));
                }
            }
            None
        })
        .collect();

    let param_names: Vec<_> = params.iter().map(|(n, _, _)| n).collect();
    let param_types: Vec<_> = params.iter().map(|(_, t, _)| t).collect();
    let param_docs: Vec<_> = params.iter().map(|(_, _, d)| d).collect();
    let fn_doc = extract_doc_comment(&input.attrs);
    let visibility = &input.vis;
    let block = &input.block;
    let ret_ty = match &input.sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    let destruct = quote! {
        let #args_struct_name { #(#param_names),* } = args;
    };

    let expanded = quote! {
        #[derive(serde::Deserialize, schemars::JsonSchema, Clone, Debug)]
        #visibility struct #args_struct_name {
            #(
                #[doc = #param_docs]
                pub #param_names: #param_types,
            )*
        }

        impl motif::ToolArgs for #args_struct_name {
            const TOOL_NAME: &'static str = stringify!(#fn_name);
            const TOOL_DESCRIPTION: &'static str = #fn_doc;
        }

        #visibility async fn #fn_name(
            #(self,)?
            args: #args_struct_name
        ) -> #ret_ty {
            #destruct
            #block
        }
    };

    TokenStream::from(expanded)
}
