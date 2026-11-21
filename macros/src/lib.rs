use proc_macro::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Block, Ident, Index, Item, ItemFn, LitBool, LitStr, Stmt, parse::{Parse, ParseStream}, parse_macro_input};

struct ModuleInput {
    name: LitStr,
    content: Block,
}

impl Parse for ModuleInput {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let name: LitStr = input.parse()?; // parse the string literal
        let _comma: syn::Token![,] = input.parse()?; // require a comma
        let content: Block = input.parse()?; // parse the block
        Ok(ModuleInput { name, content })
    }
}

#[proc_macro]
pub fn module(input: TokenStream) -> TokenStream {
    let ModuleInput { name, content } = parse_macro_input!(input as ModuleInput);

    let mut output_items = Vec::new();

    for stmt in content.stmts {
        if let Stmt::Item(Item::Fn(func)) = stmt {
            let decorated = quote! {
                #[nlf_macros::new_expose(#name)]
                #[nlf_macros::native_fn("nlf_shared", false)]
                #func
            };
            output_items.push(decorated);
        } else {
            // leave other items unchanged
            output_items.push(quote! { #stmt });
        }
    }

    TokenStream::from(quote! { #(#output_items)* })
}

#[proc_macro_attribute]
pub fn global(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    quote! {
        #[nlf_macros::new_expose]
        #[nlf_macros::native_fn("nlf_shared", false)]
        #input
    }.into()
}

struct OptionalString(Option<LitStr>);

impl Parse for OptionalString {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        if input.is_empty() {
            Ok(OptionalString(None)) // no argument passed
        } else {
            let s: LitStr = input.parse()?;
            Ok(OptionalString(Some(s)))
        }
    }
}

#[proc_macro_attribute]
pub fn expose(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let name_str = name.to_string();

    let opt = parse_macro_input!(attr as OptionalString);

    let register_fn = if let Some(lit) = opt.0 {
        quote! {
            let mut map = crate::stdlib::MODULE_TABLE.lock();
            map.entry(#lit).or_insert_with(std::collections::HashMap::new).insert(
                #name_str,
                #name
            );
        }
    } else {
        quote! {
            let mut map = crate::stdlib::FUNCTION_TABLE.lock();
            map.insert(#name_str, #name);
        }
    };

    let register_name = format_ident!("_register_{}", name);

    let expanded = quote! {
        #input

        #[ctor::ctor]
        fn #register_name() {
            #register_fn
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn new_expose(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let name_str = name.to_string();

    let opt = parse_macro_input!(attr as OptionalString);

    let register_fn = if let Some(lit) = opt.0 {
        quote! {
            let mut map = crate::stdlib::MODULE_TABLE.lock();
            map.entry(#lit).or_insert_with(std::collections::HashMap::new).insert(
                #name_str,
                #name
            );
        }
    } else {
        quote! {
            let mut map = crate::stdlib::FUNCTION_TABLE.lock();
            map.insert(#name_str, #name);
        }
    };

    let register_name = format_ident!("_register_{}", name);

    let expanded = quote! {
        #input

        #[ctor::ctor]
        fn #register_name() {
            #register_fn
        }
    };

    expanded.into()
}

fn get_type_ident(input: &syn::Type) -> &Ident {
    match input {
        syn::Type::Path(ty) => &ty.path.segments.first().unwrap().ident,
        _ => panic!(),
    }
}

struct NativeFnAttr {
    shared_crate_path: syn::Path,
    is_extern: bool
}

impl Parse for NativeFnAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let shared_crate_lit: LitStr = input.parse()?;
        let shared_crate_path: syn::Path = syn::parse_str(&shared_crate_lit.value()).unwrap();

        let _: syn::Token![,] = input.parse()?;

        let is_extern: LitBool = input.parse()?;

        Ok(NativeFnAttr { shared_crate_path, is_extern: is_extern.value })
    }
}

#[proc_macro_attribute]
pub fn native_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident;
    let name_str = name.to_string();

    let NativeFnAttr { shared_crate_path, is_extern } = parse_macro_input!(attr as NativeFnAttr);

    let argument_count = input
        .sig
        .inputs
        .len();

    let types: &Vec<syn::Type> = &input
        .sig
        .inputs
        .clone()
        .into_iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            if index == argument_count - 1 {
                return None
            }

            match arg {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(syn::PatType { ty, .. }) => Some(*ty),
            }
        })
        .collect();

    let mut mapped_types = vec![];

    let indices: Vec<Index> = (0..types.len())
        .map(|i| Index { index: i as u32, span: Span::call_site().into() })
        .collect();

    let mut names: Vec<_> = indices.clone().into_iter().map(|i| format_ident!("arg{}", i)).collect();

    names.push(Ident::new("context", Span::call_site().into()));

    for (index, ty) in types.iter().enumerate() {
        let ident = get_type_ident(ty).to_string();

        mapped_types.push(match ident.as_str() {
            "Value" => {
                let name = &names[index];
                quote! {
                    let #name = context.pop_value();
                }
            }
            "String" => {
                let name = &names[index];
                quote! {
                    let #name = {
                        let string_id = context.pop_string_id();
                        context.get_string(string_id).to_string()
                    };
                }
            }
            "f64" => {
                let name = &names[index];
                quote! {
                    let #name = context.pop_float();
                }
            }
            _ => todo!()
        });
    }

    let mapped_return = match &input.sig.output {
        syn::ReturnType::Default => {
            quote! {
                context.push_value(nlf_shared::vm::Value::Void);
            }
        },
        syn::ReturnType::Type(_, _ty) => {
            // let ident = get_type_ident(ty).to_string();

            // match ident.as_str() {
            //     "Value" => {
            //         quote! {
            //             context.push_value(return_value);
            //         }
            //     }
            //     _ => todo!()
            // }

            quote! {
                context.push_value(return_value?);
            }
        }
    };

    let new_name_str = format!("{}_impl", name_str);
    let new_ident = Ident::new(&new_name_str, name.span());

    input.sig.ident = new_ident.clone();

    let body = quote! {
        #input

        #(#mapped_types)*
        
        let return_value = #new_ident(#(#names),*);

        #mapped_return

        Ok(())
    };

    let expanded = if is_extern {
        quote! {
            #[unsafe(no_mangle)]
            extern "Rust" fn #name(context: #shared_crate_path::vm::VMContext) -> #shared_crate_path::errors::LanguageResult<()> {
                let _ = crate::__ADDON_INIT_MARKER;
                
                #body
            }
        }
    } else {
        quote! {
            pub fn #name(context: #shared_crate_path::vm::VMContext) -> #shared_crate_path::errors::LanguageResult<()> {
                #body
            }
        }
    };

    expanded.into()
}