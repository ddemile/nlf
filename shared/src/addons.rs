use crate::{errors::LanguageResult, vm::VMContext};

#[derive(Clone)]
pub struct AddonMetadata {
    pub nlf_version: &'static str
}

pub type NewAddonCall = extern "Rust" fn(VMContext) -> LanguageResult<()>;

#[derive(Clone)]
pub struct NewAddonFn {
    pub name: &'static str,
    pub call: NewAddonCall
}

pub struct NewRegistry {
    pub functions: Vec<NewAddonFn>
}

impl NewRegistry {
    pub fn new() -> Self {
        return Self {
            functions: vec![]
        }
    }

    pub fn register(&mut self, function: NewAddonFn) {
        self.functions.push(function);
    }
}