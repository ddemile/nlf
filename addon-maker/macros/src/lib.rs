
use proc_macro::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Index, ItemFn, parse_macro_input};

fn get_type_ident(input: &syn::Type) -> &Ident {
    match input {
        syn::Type::Path(ty) => ty.path.get_ident().unwrap(),
        _ => panic!(),
    }
}

fn convert_type(input: &syn::Type) -> Ident {
    let ident = get_type_ident(input).to_string();
    let kind = match ident.as_str() {
        "DynamicNumber" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" => "Number",
        "String" => "String",
        "bool" => "Bool",
        "FnRef" => "Function",
        _ => panic!()
    };

    Ident::new(kind, Span::call_site().into())
}

#[proc_macro_attribute]
pub fn register(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
            addon_maker::AddonValue::#return_type
        }
    } else if return_type.to_string() == "Number" {
        quote! {
            addon_maker::AddonValue::#return_type(DynamicNumber::from(#fn_call))
        }
    } else {
        quote! {            
            addon_maker::AddonValue::#return_type(#fn_call)
        }
    };

    let register_fn = quote! {
        #[unsafe(no_mangle)]
        pub extern "Rust" fn #name(values: Vec<addon_maker::AddonValue>) -> addon_maker::AddonValue {
            let _ = crate::__ADDON_INIT_MARKER;

            #(
                let Some(addon_maker::AddonValue::#mapped_types(#names)) = values.get(#indices) else {
                    panic!("Expected {}", stringify!(#mapped_types))
                }
            );*;
            #call
        }

        // register metadata
        addon_maker::inventory::submit! {
            addon_maker::AddonFn {
                name: #name_str,
                call: #name
            }
        }
    };

    let expanded = quote! {
        #input

        #register_fn
    };

    expanded.into()
}

#[proc_macro]
pub fn init(_item: TokenStream) -> TokenStream {
    quote! {
        pub const __ADDON_INIT_MARKER: () = ();

        #[unsafe(no_mangle)]
        pub extern "Rust" fn all_registered() -> Vec<&'static addon_maker::AddonFn> {
            addon_maker::inventory::iter::<addon_maker::AddonFn>.into_iter().collect()
        }
    }.into()
}