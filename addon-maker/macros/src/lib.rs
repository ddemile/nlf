use proc_macro::{TokenStream};
use quote::{quote};
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn addon_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    quote! {
        #[nlf_addon_maker::shared_macros::native_fn("nlf_addon_maker::shared", true)]
        #[inline(always)]
        #input
    }.into()
}