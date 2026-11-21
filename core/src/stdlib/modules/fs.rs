use std::fs;

use nlf_macros::module;

use crate::{errors::LanguageError, errors::RuntimeError};
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

module!("fs", {
    fn read(path: String, context: VMContext) -> LanguageResult<Value> {
        fs::read_to_string(&path)
            .map(|contents| Value::String(context.heap().allocate_string(contents)))
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to read {path}",))))
    }

    fn write(path: String, contents: String, _: VMContext) -> LanguageResult<Value> {
        fs::write(&path, contents)
            .map(|_| Value::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to write {path}"))))
    }

    fn rm(path: String, _: VMContext) -> LanguageResult<Value> {
        fs::remove_file(&path)
            .map(|_| Value::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to delete file {path}",))))
    }

    fn rmdir(path: String, _: VMContext) -> LanguageResult<Value> {
        fs::remove_dir_all(&path)
            .map(|_| Value::Void)
            .map_err(|_| LanguageError::from(RuntimeError::Custom(format!("Failed to delete dir {path}",))))
    }
});