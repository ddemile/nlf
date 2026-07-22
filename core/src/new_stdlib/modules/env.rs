use std::env;

use nlf_macros::new_module;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

use crate::{errors::LanguageError, interpreter::RuntimeError};

new_module!("env", {
    fn get(variable: String, context: VMContext) -> LanguageResult<Value> {
        env::var(&variable)
            .map(|value| Value::String(context.heap().allocate_string(value)))
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Environment variable '{}' not found", variable))))
    }

    fn set(variable: String, value: String, _: VMContext) -> LanguageResult<Value> {
        unsafe {
            env::set_var(&variable, &value);
        }
        Ok(Value::Void)
    }
});