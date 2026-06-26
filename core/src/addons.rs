use std::{collections::HashMap, path::PathBuf};

use lazy_static::lazy_static;
use libloading::{Library, Symbol};
use parking_lot::Mutex;
use nlf_shared::{NLF_VERSION, addons::{AddonCall, AddonMetadata, AddonValue, FnRef, ObjectRef as AddonObjectRef, Registry}};

use crate::{interpreter::eval_runtime_function, parser::{ArrayRef, FunctionKind, ObjectRef, ValueHolder}};

impl From<ValueHolder> for AddonValue {
    fn from(value: ValueHolder) -> Self {
        match value {
            ValueHolder::Number(number) => AddonValue::Number(number),
            ValueHolder::String(string) => AddonValue::String(string),
            ValueHolder::Bool(bool) => AddonValue::Bool(bool),
            ValueHolder::Fn(func) => {
                let func = func.clone();
                let function_ref = FnRef::new(move |args| {
                    let FunctionKind::Runtime(function) = func.clone() else {
                        panic!()
                    };

                    let mut converted_args: Vec<ValueHolder> = vec![];

                    for arg in args {
                        converted_args.push(arg.into());
                    }

                    let context = function.context;

                    eval_runtime_function(function, &converted_args, unsafe { &mut *context }).unwrap().into()
                });

                AddonValue::Function(function_ref)
            },
            ValueHolder::Array(array_ref) => AddonValue::Array(array_ref.fetch().iter().map(|value| (*value).clone().into()).collect()),
            ValueHolder::Object(object_ref) => AddonValue::Object(AddonObjectRef {
                object: object_ref.fetch().iter().map(|(key, value)| (key.clone(), value.clone().into())).collect(),
                schema_store: object_ref.schema_store
            }),
            ValueHolder::Void => AddonValue::Void,
            _ => panic!()
        }
    }
}

impl Into<ValueHolder> for AddonValue {
    fn into(self) -> ValueHolder {
        match self {
            AddonValue::Number(number) => ValueHolder::Number(number),
            AddonValue::String(string) => ValueHolder::String(string),
            AddonValue::Bool(bool) => ValueHolder::Bool(bool),
            AddonValue::Array(array) => ValueHolder::Array(ArrayRef::new(array.iter().map(|value| value.clone().into()).collect())),
            AddonValue::Object(object_ref) => ValueHolder::Object(ObjectRef::new(
                object_ref.object.iter().map(|(key, value)| (key.clone(), value.clone().into())).collect(),
                None,
                object_ref.schema_store
            )),
            AddonValue::Void => ValueHolder::Void,
            _ => panic!()
        }
    }
}

pub struct Addon {
    pub path: PathBuf,
    pub functions: HashMap<String, AddonCall>
}

lazy_static! {
    static ref LIBRARIES: Mutex<Vec<Library>> = Mutex::new(vec![]);
}

impl Addon {
    pub fn new(path: &str) -> Self {
        let mut map = HashMap::new();
        
        unsafe {
            let lib: Library = Library::new(path).unwrap();

            let mut registry = Registry::new();

            let metadata_function: Symbol<unsafe extern "Rust" fn() -> AddonMetadata> =
                lib.get(b"metadata").unwrap();

            let metadata = metadata_function();

            if metadata.nlf_version != NLF_VERSION {
                panic!("Tried to load an incompatible addon\nCurrent: {}\nAddon: {}", NLF_VERSION, metadata.nlf_version)
            }

            let register_functions: Symbol<unsafe extern "Rust" fn(&mut Registry)> =
                lib.get(b"register_functions").unwrap();

            register_functions(&mut registry);

            for function in registry.functions {
                map.insert(function.name.to_string(), function.call);
            }

            LIBRARIES.lock().push(lib);

            Addon { path: path.into(), functions: map }
        }
    }

    pub fn call(&self, name: &str, values: Vec<AddonValue>) -> AddonValue {
        self.functions.get(name).unwrap()(values)
    }
}