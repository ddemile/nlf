use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use lazy_static::lazy_static;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::{Expression, ValueHolder}};

#[derive(Eq, Hash, PartialEq, Debug)]
pub enum Operation {
    Addition,
    Substraction,
    Multiplication,
    Division,
    Modulo
}

pub type OperatorFunc = Box<dyn Fn(&ValueHolder, &ValueHolder) -> RuntimeResult + Send + Sync>;
pub type MethodFunc = Arc<dyn Fn(&ValueHolder, Vec<ValueHolder>, Rc<RefCell<ModuleContext>>) -> RuntimeResult + Send + Sync>;

pub struct Prototype {
    pub _name: String,
    operators: HashMap<Operation, OperatorFunc>,
    methods: HashMap<String, MethodFunc>
}

impl Prototype {
    pub fn new(name: &str) -> Self {
        Self { _name: name.to_string(), operators: HashMap::new(), methods: HashMap::new() }
    }

    pub fn with_operator(&mut self, operation: Operation, func: OperatorFunc) -> &mut Self {
        self.operators.insert(operation, func);
        
        self
    }

    pub fn with_method(&mut self, name: &str, func: MethodFunc) -> &mut Self {
        self.methods.insert(name.to_string(), func);
        self
    }

    pub fn operate(&self, operation: Operation, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult {
        let func = self.operators.get(&operation).ok_or(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))))?;

        func(a, b)
    }

    pub fn get_method(&self, name: &str) -> Option<&MethodFunc> {
        self.methods.get(name)
    }
}

lazy_static! {
    pub static ref OBJECT_PROTOTYPE: Prototype = {
        let mut prototype = Prototype::new("Object");
        prototype
            .with_method("toString", Arc::new(|instance, args, _| {
                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                Ok(ValueHolder::String(instance.to_string()))
            }));
        prototype
    };

    pub static ref ARRAY_PROTOTYPE: Prototype = {
        let mut prototype = Prototype::new("Array");
        prototype
            .with_method("toString", Arc::new(|instance, args, _| {
                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                Ok(ValueHolder::String(instance.to_string()))
            }))
            .with_method("len", Arc::new(|instance, args, context_ref| {
                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                let ValueHolder::Array(array) = instance else {
                    unreachable!()
                };

                Ok(ValueHolder::Int(array.fetch(context_ref).len() as i32))
            }));
        prototype
    };

    pub static ref STRING_PROTOTYPE: Prototype = {
        let mut prototype = Prototype::new("String");
        prototype
            .with_operator(Operation::Addition, Box::new(|a, b| {
                let ValueHolder::String(value) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected string".to_string())))
                };

                let mut value = value.to_owned();

                value.push_str(&b.to_string());
        
                Ok(ValueHolder::String(value))
            }))
            .with_method("upper", Arc::new(|instance, args, _| {
                let ValueHolder::String(string) = instance else {
                    unreachable!()
                };

                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                Ok(ValueHolder::String(string.to_uppercase()))
            }))
            .with_method("lower", Arc::new(|instance, args, _| {
                let ValueHolder::String(string) = instance else {
                    unreachable!()
                };

                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                Ok(ValueHolder::String(string.to_lowercase()))
            }));
        prototype
    };

    pub static ref INT_PROTOTYPE: Prototype = {
        let mut prototype = Prototype::new("Int");
        prototype
            .with_operator(Operation::Addition, Box::new(|a, b| {
                let ValueHolder::Int(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected int".to_string())))
                };

                let b = match b {
                    ValueHolder::Int(value) => value,
                    ValueHolder::Float(value) => return Ok(ValueHolder::Float(*a as f64 + *value)),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Int(a + b))
            }))
            .with_operator(Operation::Substraction, Box::new(|a, b| {
                let ValueHolder::Int(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected int".to_string())))
                };

                let b = match b {
                    ValueHolder::Int(value) => value,
                    ValueHolder::Float(value) => return Ok(ValueHolder::Float(*a as f64 - *value)),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Int(a - b))
            }))
            .with_operator(Operation::Multiplication, Box::new(|a, b| {
                let ValueHolder::Int(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected int".to_string())))
                };

                let b = match b {
                    ValueHolder::Int(value) => value,
                    ValueHolder::Float(value) => return Ok(ValueHolder::Float(*a as f64 * *value)),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Int(a * b))
            }))
            .with_operator(Operation::Division, Box::new(|a, b| {
                let ValueHolder::Int(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected int".to_string())))
                };

                let b = match b {
                    ValueHolder::Int(value) => value,
                    ValueHolder::Float(value) => return Ok(ValueHolder::Float(*a as f64 / *value)),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Int(a / b))
            }))
            .with_operator(Operation::Modulo, Box::new(|a, b| {
                let ValueHolder::Int(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected int".to_string())))
                };

                let b = match b {
                    ValueHolder::Int(value) => value,
                    ValueHolder::Float(value) => return Ok(ValueHolder::Float(*a as f64 / *value)),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Int(a % b))
            }));
        prototype
    };

    pub static ref FLOAT_PROTOTYPE: Prototype = {
        let mut prototype = Prototype::new("Float");
        prototype
            .with_operator(Operation::Addition, Box::new(|a, b| {
                let ValueHolder::Float(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected float".to_string())))
                };

                let b = match b {
                    ValueHolder::Float(value) => value,
                    ValueHolder::Int(value) => &(*value as f64),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Float(a + b))
            }))
            .with_operator(Operation::Substraction, Box::new(|a, b| {
                let ValueHolder::Float(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected float".to_string())))
                };

                let b = match b {
                    ValueHolder::Float(value) => value,
                    ValueHolder::Int(value) => &(*value as f64),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Float(a - b))
            }))
            .with_operator(Operation::Multiplication, Box::new(|a, b| {
                let ValueHolder::Float(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected float".to_string())))
                };

                let b = match b {
                    ValueHolder::Float(value) => value,
                    ValueHolder::Int(value) => &(*value as f64),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Float(a * b))
            }))
            .with_operator(Operation::Division, Box::new(|a, b| {
                let ValueHolder::Float(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected float".to_string())))
                };

                let b = match b {
                    ValueHolder::Float(value) => value,
                    ValueHolder::Int(value) => &(*value as f64),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Float(a / b))
            }))
            .with_operator(Operation::Modulo, Box::new(|a, b| {
                let ValueHolder::Float(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected float".to_string())))
                };

                let b = match b {
                    ValueHolder::Float(value) => value,
                    ValueHolder::Int(value) => &(*value as f64),
                    _ => return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Float(a % b))
            }));
        prototype
    };
}
