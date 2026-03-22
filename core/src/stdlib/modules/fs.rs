use std::fs;

use nlf_macros::module;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, argument};

module!("fs", {
    fn read(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let path = argument!(values, ValueHolder::String, "path", 0);

        fs::read_to_string(&path)
            .map(|contents| ValueHolder::String(contents))
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to read {path}",))))
    }

    fn write(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let path = argument!(values, ValueHolder::String, "path", 0);
        let contents = argument!(values, ValueHolder::String, "contents", 1);

        fs::write(&path, contents)
            .map(|_| ValueHolder::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to write {path}",))))
    }

    fn rm(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let path = argument!(values, ValueHolder::String, "path", 0);

        fs::remove_file(&path)
            .map(|_| ValueHolder::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to delete file {path}",))))
    }

    fn rmdir(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let path = argument!(values, ValueHolder::String, "path", 0);

        fs::remove_dir_all(&path)
            .map(|_| ValueHolder::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to delete dir {path}",))))
    }
});