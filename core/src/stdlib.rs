use std::collections::HashMap;

use lazy_static::lazy_static;
use nlf_shared::{errors::LanguageResult, vm::VMContext};
use parking_lot::Mutex;
use rust_embed::Embed;

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

pub type NativeFunctionType = fn(VMContext) -> LanguageResult<()>;

mod global;
mod modules;