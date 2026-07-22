use nlf_macros::new_module;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

use crate::{errors::LanguageError, interpreter::RuntimeError};

new_module!("http", {
    fn get(url: String, context: VMContext) -> LanguageResult<Value> {
        reqwest::blocking::get(url)
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("HTTP request failed: {}", e))))?
            .text()
            .map(|text| Value::String(context.heap().allocate_string(text)))
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("Failed to read response text: {}", e))))
    }
});