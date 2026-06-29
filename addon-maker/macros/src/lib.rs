use proc_macro::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Index, ItemFn, parse_macro_input};

fn get_type_ident(input: &syn::Type) -> &Ident {
    match input {
        syn::Type::Path(ty) => &ty.path.segments.first().unwrap().ident,
        _ => panic!(),
    }
}

fn convert_type(input: &syn::Type) -> Ident {
    let ident = get_type_ident(input).to_string();
    let kind = match ident.as_str() {
        "f64" => "Number",
        "String" => "String",
        "bool" => "Bool",
        "FnRef" => "Function",
        "AddonValue" => "AddonValue",
        "ObjectRef" => "Object",
        _ => panic!()
    };

    Ident::new(kind, Span::call_site().into())
}

#[proc_macro_attribute]
pub fn addon_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident;
    let name_str = name.to_string();

    let types: &Vec<syn::Type> = &input
        .sig
        .inputs
        .clone()
        .into_iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(syn::PatType { ty, .. }) => Some(*ty),
        })
        .collect();
    
    let return_type = match &input.sig.output {
        syn::ReturnType::Default => Ident::new("Void", Span::call_site().into()),
        syn::ReturnType::Type(_, t) => convert_type(t)
    };

    let mapped_types: Vec<Ident> = types.iter().map(|t| convert_type(t)).collect();

    let new_name_str = format!("{}_impl", name_str);
    let new_ident = Ident::new(&new_name_str, name.span());

    input.sig.ident = new_ident.clone();

    let indices: Vec<Index> = (0..types.len())
        .map(|i| Index { index: i as u32, span: Span::call_site().into() })
        .collect();

    let names: Vec<_> = indices.clone().into_iter().map(|i| format_ident!("arg{}", i)).collect();

    let fn_call = quote! { #new_ident(#( #names.clone().into() ),*) };
    let call = if return_type.to_string() == "Void" {
        quote! {
            #fn_call;
            nlf_addon_maker::AddonValue::#return_type
        }
    } else if return_type.to_string() == "AddonValue" {
        quote! {
            #fn_call
        }
    } else {
        quote! {            
            nlf_addon_maker::AddonValue::#return_type(#fn_call)
        }
    };

    let register_fn = quote! {
        #[unsafe(no_mangle)]
        extern "Rust" fn #name(values: Vec<nlf_addon_maker::AddonValue>) -> nlf_addon_maker::AddonValue {
            let _ = crate::__ADDON_INIT_MARKER;

            #(
                let Some(nlf_addon_maker::AddonValue::#mapped_types(#names)) = values.get(#indices) else {
                    panic!("Expected {}", stringify!(#mapped_types))
                }
            );*;
            #call
        }
    };

    let expanded = quote! {
        #input

        #register_fn
    };

    expanded.into()
}