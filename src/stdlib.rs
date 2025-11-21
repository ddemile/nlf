use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lazy_static::lazy_static;
use rust_embed::Embed;

use crate::{interpreter::{ModuleContext, RuntimeResult}, parser::ValueHolder};

#[derive(Embed)] 
#[folder = "src/stdlib/modules"] 
#[include = "*.nlf"]
pub struct CoreModules;

lazy_static! {
    pub static ref FUNCTION_TABLE: std::sync::Mutex<HashMap<&'static str, NativeFunctionType>> =
        std::sync::Mutex::new(HashMap::new());

    pub static ref MODULE_TABLE: std::sync::Mutex<HashMap<&'static str, HashMap<&'static str, NativeFunctionType>>> =
        std::sync::Mutex::new(HashMap::new());
}

pub type NativeFunctionType = fn(&[ValueHolder], Rc<RefCell<ModuleContext>>) -> RuntimeResult;

mod global;
mod modules;