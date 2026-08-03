use std::fmt;

pub use nlf_shared::errors::*;

#[derive(Debug)]
pub enum RuntimeError {
    InvalidType(String),
    Custom(String),
    VariableNotFound(String),
    NoSuchProperty(String),
    OperationNotSupported(String),
}

impl LanguageErrorKind for RuntimeError {}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidType(value) => write!(f, "[InvalidType] {}", value),
            RuntimeError::Custom(value) => write!(f, "[CustomError] {}", value),
            RuntimeError::VariableNotFound(name) => write!(f, "[VariableNotFound] {}", name),
            RuntimeError::NoSuchProperty(name) => {
                write!(f, "[NoSuchProperty] {}", name)
            }
            RuntimeError::OperationNotSupported(name) => {
                write!(f, "[OperationNotSupported] {}", name)
            }
        }
    }
}

#[derive(Debug)]
pub enum LoaderError {
    ModuleNotFound(String),
    ImportNotFound(String),
    BoundsViolation(String),
    TODO
}

impl LanguageErrorKind for LoaderError {}