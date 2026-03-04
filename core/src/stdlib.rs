use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lazy_static::lazy_static;
use parking_lot::Mutex;
use rust_embed::Embed;

use crate::{interpreter::{ModuleContext, RuntimeResult}, parser::ValueHolder};

#[derive(Embed)] 
#[folder = "src/stdlib/modules"] 
#[include = "*.nlf"]
pub struct CoreModules;

lazy_static! {
    pub static ref FUNCTION_TABLE: Mutex<HashMap<&'static str, NativeFunctionType>> =
        Mutex::new(HashMap::new());

    pub static ref MODULE_TABLE: Mutex<HashMap<&'static str, HashMap<&'static str, NativeFunctionType>>> =
        Mutex::new(HashMap::new());
}

pub type NativeFunctionType = fn(&[ValueHolder], Rc<RefCell<ModuleContext>>) -> RuntimeResult;

mod global;
mod modules;

#[macro_export]
macro_rules! argument {
    ($args:expr, $ctor:path, $name:expr, $idx:expr) => {{
        match $args.get($idx) {
            Some(v) => match v {
                $ctor(inner) => inner.clone(),
                _ => return Err(LanguageError::from(RuntimeError::Custom(format!(
                    "argument `{}` at index {} had wrong type",
                    $name, $idx
                )))),
            },
            None => return Err(LanguageError::from(RuntimeError::Custom(format!(
                "argument `{}` at index {} missing",
                $name, $idx
            )))),
        }
    }};
}