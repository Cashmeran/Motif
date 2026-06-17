use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Pat, ReturnType};

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
/// ```
#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let args_struct_name =
        syn::Ident::new(&format!("{}Args", to_pascal_case(&fn_name.to_string())), fn_name.span());

    let params: Vec<_> = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                    let name = &pat_ident.ident;
                    let ty = &pat_type.ty;
                    let doc = extract_doc_comment(&pat_type.attrs, &pat_ident.ident);
                    return Some((name.clone(), ty.clone(), doc));
                }
            }
            None
        })
        .collect();

    let param_names: Vec<_> = params.iter().map(|(n, _, _)| n).collect();
    let param_types: Vec<_> = params.iter().map(|(_, t, _)| t).collect();
    let param_docs: Vec<_> = params.iter().map(|(_, _, d)| d).collect();

    let fn_doc = extract_doc_comment(&input.attrs, &fn_name);
    let visibility = &input.vis;
    let block = &input.block;
    let is_async = input.sig.asyncness.is_some();
    let ret_ty = match &input.sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

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

fn extract_doc_comment(attrs: &[syn::Attribute], fallback: &syn::Ident) -> String {
    let mut doc = String::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(mnv) = &attr.meta {
                if let syn::Expr::Lit(lit) = &mnv.value {
                    if let syn::Lit::Str(s) = &lit.lit {
                        if !doc.is_empty() { doc.push('\n'); }
                        doc.push_str(s.value().trim());
                    }
                }
            }
        }
    }
    if doc.is_empty() { fallback.to_string() } else { doc }
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
