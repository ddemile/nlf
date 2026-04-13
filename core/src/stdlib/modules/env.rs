use std::env;

use nlf_macros::module;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, argument};

module!("env", {
    fn get(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let variable = argument!(values, ValueHolder::String, "variable", 0);

        env::var(&variable)
            .map(|value| ValueHolder::String(value))
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Environment variable '{}' not found", variable))))
    }

    fn set(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let variable = argument!(values, ValueHolder::String, "variable", 0);
        let value = argument!(values, ValueHolder::String, "value", 1);

        unsafe {
            env::set_var(&variable, &value);
        }
        Ok(ValueHolder::Void)
    }
});