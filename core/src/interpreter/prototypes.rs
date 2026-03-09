use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use lazy_static::lazy_static;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult, Scope}, lexer::TokenKind, parser::ValueHolder};

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub enum Operation {
    Addition,
    Substraction,
    Multiplication,
    Division,
    Modulo
}

#[derive(Clone)]
pub enum Method {
    BuiltIn(BuiltInMethodFunc),
    Local(LocalMethodFunc, Rc<RefCell<Scope>>)
}

impl Method {
    pub fn call(&self, this: &ValueHolder, arguments: Vec<ValueHolder>, context_ref: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        match self {
            Method::Local(method, scope) => method.call(this, arguments, context_ref, scope.clone()),
            Method::BuiltIn(method) => method(this, arguments, context_ref),
        }
    }
}

pub trait Prototype {
    fn operate(&self, operation: Operation, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult;
    fn get_method(&self, name: &str) -> Option<Method>;
}

#[derive(Clone)]
pub struct LocalPrototype {
    pub _name: String,
    operators: Vec<Option<LocalOperatorFunc>>,
    methods: HashMap<String, (LocalMethodFunc, Rc<RefCell<Scope>>)>
}

impl std::fmt::Debug for LocalPrototype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prototype")
            .field("_name", &self._name)
            .finish()
    }
}

impl Prototype for LocalPrototype {
    fn operate(&self, operation: Operation, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult {
        let operation_id = operation as usize;

        if self.operators.len() <= operation_id {
            return Err(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))));
        }

        let func: &LocalOperatorFunc = self.operators[operation_id].as_ref().ok_or(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))))?;

        func.call(a, b)
    }

    fn get_method(&self, name: &str) -> Option<Method> {
        self.methods.get(name).map(|(method, scope)| Method::Local(method.clone(), scope.clone()))
    }
}

pub trait LocalOperatorFuncTrait {
    fn call(&self, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult;
    fn box_clone(&self) -> Box<dyn LocalOperatorFuncTrait>;
}

impl<T> LocalOperatorFuncTrait for T
where
    T: Fn(&ValueHolder, &ValueHolder) -> RuntimeResult + Clone + 'static,
{
    fn call(&self, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult {
        self(a, b)
    }

    fn box_clone(&self) -> Box<dyn LocalOperatorFuncTrait> {
        Box::new(self.clone())
    }
}

pub type LocalOperatorFunc = Box<dyn LocalOperatorFuncTrait>;

impl Clone for LocalOperatorFunc {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait LocalMethodFuncTrait {
    fn call(&self, this: &ValueHolder, arguments: Vec<ValueHolder>, context_ref: Rc<RefCell<ModuleContext>>, scope: Rc<RefCell<Scope>>) -> RuntimeResult;
    fn box_clone(&self) -> Box<dyn LocalMethodFuncTrait>;
}

impl<T> LocalMethodFuncTrait for T
where
    T: Fn(&ValueHolder, Vec<ValueHolder>, Rc<RefCell<ModuleContext>>, Rc<RefCell<Scope>>) -> RuntimeResult + Clone + 'static,
{
    fn call(&self, this: &ValueHolder, arguments: Vec<ValueHolder>, context_ref: Rc<RefCell<ModuleContext>>, scope: Rc<RefCell<Scope>>) -> RuntimeResult {
        self(this, arguments, context_ref, scope)
    }

    fn box_clone(&self) -> Box<dyn LocalMethodFuncTrait> {
        Box::new(self.clone())
    }
}

pub type LocalMethodFunc = Box<dyn LocalMethodFuncTrait>;

impl Clone for LocalMethodFunc {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

impl LocalPrototype {
    pub fn new(name: &str) -> Self {
        Self { _name: name.to_string(), operators: vec![], methods: HashMap::new() }
    }

    pub fn with_operator(&mut self, operation: Operation, func: LocalOperatorFunc) -> &mut Self {
        let operation = operation as usize;

        if self.operators.len() <= operation {
            self.operators.resize_with(operation + 1, || None);
        }

        self.operators[operation] = Some(func);
        
        self
    }

    pub fn with_method(&mut self, name: &str, func: LocalMethodFunc, scope: Rc<RefCell<Scope>>) -> &mut Self {
        self.methods.insert(name.to_string(), (func, scope));
        self
    }
}


#[derive(Clone)]
pub struct BuiltInPrototype {
    pub _name: String,
    operators: Vec<Option<BuiltInOperatorFunc>>,
    methods: HashMap<String, BuiltInMethodFunc>
}

impl Prototype for BuiltInPrototype {
    fn operate(&self, operation: Operation, a: &ValueHolder, b: &ValueHolder) -> RuntimeResult {
        let operation_id = operation as usize;

        if self.operators.len() <= operation_id {
            return Err(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))));
        }

        let func: &BuiltInOperatorFunc = self.operators[operation_id].as_ref().ok_or(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))))?;

        func(a, b)
    }

    fn get_method(&self, name: &str) -> Option<Method> {
        self.methods.get(name).map(|method| Method::BuiltIn(method.clone()))
    }
}

pub type BuiltInOperatorFunc = Arc<dyn Fn(&ValueHolder, &ValueHolder) -> RuntimeResult + Send + Sync>;
pub type BuiltInMethodFunc = Arc<dyn Fn(&ValueHolder, Vec<ValueHolder>, Rc<RefCell<ModuleContext>>) -> RuntimeResult + Send + Sync>;

impl std::fmt::Debug for BuiltInPrototype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prototype")
            .field("_name", &self._name)
            .finish()
    }
}

impl BuiltInPrototype {
    pub fn new(name: &str) -> Self {
        Self { _name: name.to_string(), operators: vec![], methods: HashMap::new() }
    }

    pub fn with_operator(&mut self, operation: Operation, func: BuiltInOperatorFunc) -> &mut Self {
        let operation = operation as usize;

        if self.operators.len() <= operation {
            self.operators.resize_with(operation + 1, || None);
        }

        self.operators[operation] = Some(func);
        
        self
    }

    pub fn with_method(&mut self, name: &str, func: BuiltInMethodFunc) -> &mut Self {
        self.methods.insert(name.to_string(), func);
        self
    }
}

lazy_static! {
    pub static ref OBJECT_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("Object");
        prototype
            .with_method("toString", Arc::new(|instance, args, _| {
                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                Ok(ValueHolder::String(instance.to_string()))
            }));
        prototype
    };

    pub static ref ARRAY_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("Array");
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

                Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Unsigned32(array.fetch(context_ref).len() as u32))))
            }))
            .with_method("push", Arc::new(|instance, args, context_ref| {
                if args.len() > 1 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                let ValueHolder::Array(array) = instance else {
                    unreachable!()
                };

                array.push(args[0].clone(), context_ref);

                Ok(ValueHolder::Void)
            }))
            .with_method("reverse", Arc::new(|instance, args, context_ref| {
                if args.len() > 0 {
                    return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
                }

                let ValueHolder::Array(array) = instance else {
                    unreachable!()
                };

                array.reverse(context_ref);

                Ok(ValueHolder::Void)
            }));
        prototype
    };

    pub static ref STRING_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("String");
        prototype
            .with_operator(Operation::Addition, Arc::new(|a, b| {
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

    pub static ref NUMBER_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("Number");
        prototype
            .with_operator(Operation::Addition, Arc::new(|a, b| {
                let ValueHolder::Number(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };

                let ValueHolder::Number(b) = b else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Number(*a + *b))
            }))
            .with_operator(Operation::Substraction, Arc::new(|a, b| {
                let ValueHolder::Number(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };

                let ValueHolder::Number(b) = b else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Number(*a - *b))
            }))
            .with_operator(Operation::Multiplication, Arc::new(|a, b| {
                let ValueHolder::Number(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };

                let ValueHolder::Number(b) = b else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Number(*a * *b))
            }))
            .with_operator(Operation::Division, Arc::new(|a, b| {
                let ValueHolder::Number(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };

                let ValueHolder::Number(b) = b else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
        
                Ok(ValueHolder::Number(*a / *b))
            }))
            .with_operator(Operation::Modulo, Arc::new(|a, b| {
                let ValueHolder::Number(a) = a else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };

                let ValueHolder::Number(b) = b else {
                    return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
                };
                
                Ok(ValueHolder::Number(*a % *b))
            }));
        prototype
    };
}