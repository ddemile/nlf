use std::{collections::HashMap, path::PathBuf};

use lazy_static::lazy_static;
use libloading::{Library, Symbol};
use parking_lot::Mutex;
use nlf_shared::addons::{AddonCall, AddonFn, AddonValue, FnRef};

use crate::{interpreter::eval_runtime_function, parser::{FunctionKind, ValueHolder}};

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

                    eval_runtime_function(function, &converted_args).unwrap().into()
                });

                AddonValue::Function(function_ref)
            },
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
            AddonValue::Void => ValueHolder::Void,
            _ => panic!()
        }
    }
}

pub struct Addon {
    path: PathBuf,
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

            let all_registered: Symbol<unsafe extern "Rust" fn() -> Vec<&'static AddonFn>> =
                lib.get(b"all_registered").unwrap();

            for function in all_registered() {
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