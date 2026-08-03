use nlf_macros::module;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

use crate::{errors::LanguageError, errors::RuntimeError};

module!("http", {
    fn get(url: String, context: VMContext) -> LanguageResult<Value> {
        reqwest::blocking::get(url)
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("HTTP request failed: {}", e))))?
            .text()
            .map(|text| Value::String(context.heap().allocate_string(text)))
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("Failed to read response text: {}", e))))
    }
});