use std::{collections::HashMap, path::PathBuf};

use lazy_static::lazy_static;
use libloading::{Library, Symbol};
use parking_lot::Mutex;
use shared::addons::{AddonCall, AddonFn, AddonValue};

use crate::{parser::ValueHolder};

impl From<ValueHolder> for AddonValue {
    fn from(value: ValueHolder) -> Self {
        match value {
            ValueHolder::Number(number) => AddonValue::Number(number),
            ValueHolder::String(string) => AddonValue::String(string),
            ValueHolder::Bool(bool) => AddonValue::Bool(bool),
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