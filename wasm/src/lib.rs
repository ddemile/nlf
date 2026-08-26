#![feature(hash_map_macro)]

extern crate console_error_panic_hook;

use std::{hash_map, panic, rc::Rc};

use nlf_macros::native_fn;
use nlf_shared::vm::VMContext;
use wasm_bindgen::prelude::*;
use nlf_core::{compiler::resolvers::{ModuleResolver, StaticModuleResolver}, errors::LanguageResult, loader::Loader, stdlib::FUNCTION_TABLE, vm::{Value, run_main}};

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
    pub fn confirm(s: &str) -> bool;
    pub fn prompt(s: &str) -> String;

    // #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    fn error(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

macro_rules! console_error {
    ($($t:tt)*) => (error(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn init_nlf() {
    panic::set_hook(Box::new(|error| {
        console_error!("{}", error)
    }));
}

#[wasm_bindgen]
pub fn run(code: &str) {
    #[native_fn("nlf_shared", false)]
    fn print_function(value: Value, context: VMContext) {
        console_log!("{}", context.stringify(&value))
    }

    #[native_fn("nlf_shared", false)]
    fn confirm_function(prompt: String, _: VMContext) -> LanguageResult<Value> {
        return Ok(Value::Bool(confirm(&prompt)))
    }

    #[native_fn("nlf_shared", false)]
    fn input_function(prompt_string: String, context: VMContext) -> LanguageResult<Value> {
        return Ok(Value::String(context.heap().allocate_string(prompt(&prompt_string))))
    }

    FUNCTION_TABLE.lock().insert("print", print_function);
    FUNCTION_TABLE.lock().insert("confirm", confirm_function);
    FUNCTION_TABLE.lock().insert("input", input_function);

    let modules = hash_map! {
        "main".to_string() => code.to_string()
    };

    let resolver: Rc<dyn ModuleResolver> = Rc::new(StaticModuleResolver {
        modules
    });

    let loader = Loader::new(String::from("main"), resolver);

    loader.run_main();
}