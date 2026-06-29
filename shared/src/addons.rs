use std::{cell::RefCell, rc::Rc};

use indexmap::IndexMap;

use crate::SchemaStore;

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

#[derive(Clone)]
pub struct ObjectRef {
    pub object: IndexMap<String, AddonValue>,
    pub schema_store: Rc<RefCell<SchemaStore>>
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

#[derive(Clone)]
pub enum AddonValue {
    String(String),
    Number(f64),
    Bool(bool),
    Function(FnRef),
    Array(Vec<AddonValue>),
    Object(ObjectRef),
    Void
}