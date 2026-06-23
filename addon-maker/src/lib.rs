pub use macros::addon_fn;
pub use nlf_shared::addons::*;
pub use nlf_shared::numbers::DynamicNumber;
pub use nlf_shared::NLF_VERSION;

#[macro_export]
macro_rules! init {
    ($( $fn_name:ident ),* ) => {
        pub const __ADDON_INIT_MARKER: () = ();
        
        pub const FUNCTIONS: std::sync::Mutex<Vec<nlf_addon_maker::AddonFn>> = Mutex::new(vec![]);

        #[unsafe(no_mangle)]
        extern "Rust" fn metadata() -> nlf_addon_maker::AddonMetadata {
            nlf_addon_maker::AddonMetadata {
                nlf_version: nlf_addon_maker::NLF_VERSION,
                // functions: FUNCTIONS.lock().unwrap().to_vec()
            }
        }

        #[unsafe(no_mangle)]
        extern "Rust" fn register_functions(registry: &mut nlf_addon_maker::Registry){
            $(
                registry.register(nlf_addon_maker::AddonFn {
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
        $registry.register(nlf_addon_maker::AddonFn {
            name: stringify!($function),
            call: $function
        })
    };
}