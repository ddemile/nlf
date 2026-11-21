pub use macros::addon_fn;
pub use nlf_shared::addons::*;
pub use nlf_shared::NLF_VERSION;
pub use nlf_shared as shared;
pub use nlf_macros as shared_macros;

#[macro_export]
macro_rules! init {
    ($( $fn_name:ident ),* ) => {
        pub const __ADDON_INIT_MARKER: () = ();
        
        #[unsafe(no_mangle)]
        extern "Rust" fn metadata() -> nlf_addon_maker::AddonMetadata {
            nlf_addon_maker::AddonMetadata {
                nlf_version: nlf_addon_maker::NLF_VERSION
            }
        }

        #[unsafe(no_mangle)]
        extern "Rust" fn register_functions(registry: &mut nlf_addon_maker::NewRegistry){
            $(
                registry.register(nlf_addon_maker::NewAddonFn {
                    name: stringify!($fn_name),
                    call: $fn_name
                });
            )*
        }
    };
}

#[macro_export]
macro_rules! register_fn {
    ($registry:expr, $function:ident) => {
        $registry.register(nlf_addon_maker::NewAddonFn {
            name: stringify!($function),
            call: $function
        })
    };
}