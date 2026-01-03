use std::{cell::RefCell, rc::Rc, sync::{Arc}};

use lib::{module};
use parking_lot::Mutex;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, argument};

module!("http", {
    fn get(values: &[ValueHolder], _context: Arc<Mutex<ModuleContext>>) -> RuntimeResult {
        let url = argument!(values, ValueHolder::String, "url", 0);

        reqwest::blocking::get(url)
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("HTTP request failed: {}", e))))?
            .text()
            .map(|text| ValueHolder::String(text))
            .map_err(|e| LanguageError::from(RuntimeError::Custom(format!("Failed to read response text: {}", e))))
    }
});