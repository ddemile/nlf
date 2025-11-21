use std::{cell::RefCell, rc::Rc};

use lib::{expose, module};

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder};

module!("http", {
    fn get(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let ValueHolder::String(url) = values.get(0).ok_or(
            LanguageError::from(
                RuntimeError::Custom("Missing argument 'url'".into())
            )
        )? else {
            return Err(LanguageError::from(
                RuntimeError::Custom("Argument 'url' must be a string".into())
            ));
        };

        reqwest::blocking::get(url)
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("HTTP request failed: {}", e))))?
            .text()
            .map(|text| ValueHolder::String(text))
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("Failed to read response text: {}", e))))
    }
});