use std::{collections::HashMap, path::PathBuf};

use lazy_static::lazy_static;
use libloading::{Library, Symbol};
use parking_lot::Mutex;
use nlf_shared::{NLF_VERSION, addons::{AddonCall, AddonMetadata, AddonValue, FnRef, NewAddonCall, NewRegistry, ObjectRef as AddonObjectRef, Registry}, errors::LanguageResult, vm::VMContext};

use crate::{interpreter::eval_runtime_function, parser::{ArrayRef, FunctionKind, ObjectRef, ValueHolder}};

pub struct Addon {
    pub path: PathBuf,
    pub functions: HashMap<String, NewAddonCall>
}

lazy_static! {
    static ref LIBRARIES: Mutex<Vec<Library>> = Mutex::new(vec![]);
}

impl Addon {
    pub fn new(path: &str) -> Self {
        let mut map = HashMap::new();
        
        unsafe {
            let lib = Library::new(path).unwrap();

            let mut registry = NewRegistry::new();

            let metadata_function: Symbol<unsafe extern "Rust" fn() -> AddonMetadata> =
                lib.get(b"metadata").unwrap();

            let metadata = metadata_function();

            if metadata.nlf_version != NLF_VERSION {
                panic!("Tried to load an incompatible addon\nCurrent: {}\nAddon: {}", NLF_VERSION, metadata.nlf_version)
            }

            let register_functions: Symbol<unsafe extern "Rust" fn(&mut NewRegistry)> =
                lib.get(b"register_functions").unwrap();

            register_functions(&mut registry);

            for function in registry.functions {
                map.insert(function.name.to_string(), function.call);
            }

            LIBRARIES.lock().push(lib);

            Addon { path: path.into(), functions: map }
        }
    }

    pub fn call(&self, name: &str, context: VMContext) -> LanguageResult<()> {
        self.functions.get(name).unwrap()(context)
    }
}