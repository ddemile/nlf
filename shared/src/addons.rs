use std::rc::Rc;

use crate::numbers::DynamicNumber;

pub type AddonCall = extern "Rust" fn(Vec<AddonValue>) -> AddonValue;

#[derive(Clone)]
pub struct AddonFn {
    pub name: &'static str,
    pub call: AddonCall
}

#[derive(Clone)]
pub struct AddonMetadata {
    pub nlf_version: &'static str
}

pub struct Registry {
    pub functions: Vec<AddonFn>
}

impl Registry {
    pub fn new() -> Self {
        return Self {
            functions: vec![]
        }
    }

    pub fn register(&mut self, function: AddonFn) {
        self.functions.push(function);
    }
}

#[derive(Clone)]
pub struct FnRef {
    pub func: Rc<dyn Fn(Vec<AddonValue>) -> AddonValue> 
}

impl FnRef {
    pub fn new(func: impl Fn(Vec<AddonValue>) -> AddonValue + 'static) -> Self {
        Self {
            func: Rc::new(func)
        }
    }

    pub fn call(&self, args: Vec<AddonValue>) -> AddonValue {
        (self.func)(args)
    }
}

pub enum AddonValue {
    String(String),
    Number(DynamicNumber),
    Bool(bool),
    Function(FnRef),
    Void
}