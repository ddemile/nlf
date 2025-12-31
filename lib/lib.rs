use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Block, Item, ItemFn, LitStr, Stmt, parse::{Parse, ParseStream}, parse_macro_input};

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
                #[lib::expose(#name)]
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

    // for arg in &input.sig.inputs {
    //     match arg {
    //         FnArg::Receiver(_) => println!("  Receiver: self"),
    //         FnArg::Typed(pat_type) => {
    //             if let Pat::Ident(pat_ident) = &*pat_type.pat {
    //                 // Use `to_token_stream` to print the type as a string
    //                 let ty_str = pat_type.ty.to_token_stream().to_string();
    //                 println!("  Argument: {} : {}", pat_ident.ident, ty_str);
    //             } else {
    //                 println!("  Other pattern argument: {}", pat_type.pat.to_token_stream());
    //             }
    //         }
    //     }
    // }

    let register_fn = if let Some(lit) = opt.0 {
        quote! {
            let mut map = crate::stdlib::MODULE_TABLE.lock().unwrap();
            map.entry(#lit).or_insert_with(std::collections::HashMap::new).insert(
                #name_str,
                #name
            );
        }
    } else {
        quote! {
            let mut map = crate::stdlib::FUNCTION_TABLE.lock().unwrap();
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