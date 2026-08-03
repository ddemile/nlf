use std::{collections::HashMap, sync::Arc};

use lazy_static::lazy_static;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

use crate::{errors::{LanguageError, RuntimeError}, vm::{RuntimeResult, ValueUtils}};

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
    Local(LocalMethodFunc)
}

impl Method {
    pub fn call(&self, this: &Value, context: VMContext) -> LanguageResult<()> {
        match self {
            Method::Local(_method) => {
                // method.call(this, arguments, context, scope.clone())
                todo!()
            },
            Method::BuiltIn(method) => method(this, context),
        }
    }
}

pub trait Prototype {
    fn operate(&self, operation: Operation, a: &Value, b: &Value) -> LanguageResult<()>;
    fn get_method(&self, name: &str) -> Option<Method>;
}

#[derive(Clone)]
pub struct LocalPrototype {
    pub _name: String,
    pub operators: Vec<Option<LocalOperatorFunc>>,
    pub methods: HashMap<String, LocalMethodFunc >
}

impl std::fmt::Debug for LocalPrototype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prototype")
            .field("_name", &self._name)
            .finish()
    }
}

impl Prototype for LocalPrototype {
    fn operate(&self, operation: Operation, _a: &Value, _b: &Value) -> LanguageResult<()> {
        let operation_id = operation as usize;

        if self.operators.len() <= operation_id {
            return Err(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))));
        }

        let _func: &LocalOperatorFunc = self.operators[operation_id].as_ref().ok_or_else(|| LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))))?;

        // func.call(a, b)

        Ok(())
    }

    fn get_method(&self, _name: &str) -> Option<Method> {
        todo!();
        // self.methods.get(name).map(|(method, scope)| Method::Local(method.clone(), scope.clone()))
    }
}

pub trait LocalOperatorFuncTrait {
    fn call(&self, a: &Value, b: &Value) -> RuntimeResult;
    fn box_clone(&self) -> Box<dyn LocalOperatorFuncTrait>;
}

impl<T> LocalOperatorFuncTrait for T
where
    T: Fn(&Value, &Value) -> RuntimeResult + Clone + 'static,
{
    fn call(&self, a: &Value, b: &Value) -> RuntimeResult {
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
    fn call(&self, this: &Value, arguments: &[Value]) -> RuntimeResult;
    fn box_clone(&self) -> Box<dyn LocalMethodFuncTrait>;
}

impl<T> LocalMethodFuncTrait for T
where
    T: Fn(&Value, &[Value]) -> RuntimeResult + Clone + 'static,
{
    fn call(&self, this: &Value, arguments: &[Value]) -> RuntimeResult {
        self(this, arguments)
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

    pub fn with_method(&mut self, name: &str, func: LocalMethodFunc) -> &mut Self {
        self.methods.insert(name.to_string(), func );
        self
    }
}


#[derive(Clone)]
pub struct BuiltInPrototype {
    pub _name: String,
    pub operators: Vec<Option<BuiltInOperatorFunc>>,
    pub methods: HashMap<String, BuiltInMethodFunc>
}

impl Prototype for BuiltInPrototype {
    fn operate(&self, operation: Operation, a: &Value, b: &Value) -> LanguageResult<()> {
        let operation_id = operation as usize;

        if self.operators.len() <= operation_id {
            return Err(LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))));
        }

        let func: &BuiltInOperatorFunc = self.operators[operation_id].as_ref().ok_or_else(|| LanguageError::from(RuntimeError::OperationNotSupported(format!("{:?}", operation))))?;

        func(a, b)
    }

    fn get_method(&self, name: &str) -> Option<Method> {
        self.methods.get(name).map(|method| Method::BuiltIn(method.clone()))
    }
}

pub type BuiltInOperatorFunc = Arc<dyn Fn(&Value, &Value) -> LanguageResult<()> + Send + Sync>;
pub type BuiltInMethodFunc = Arc<dyn Fn(&Value, VMContext) -> LanguageResult<()> + Send + Sync>;

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
            .with_method("toString", Arc::new(|instance, context| {
                let string = ValueUtils::to_string(instance, context.heap());

                let string_id = context.heap().allocate_string(string);

                context.push_value(Value::String(string_id));

                Ok(())
            }));
        prototype
    };

    pub static ref ARRAY_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("Array");
        prototype
            .with_method("toString", Arc::new(|instance, context| {
                let string = ValueUtils::to_string(instance, context.heap());

                let string_id = context.heap().allocate_string(string);

                context.push_value(Value::String(string_id));

                Ok(())
            }))
            .with_method("len", Arc::new(|instance, context| {
                let Value::Array(array_id) = instance else {
                    unreachable!()
                };

                let array = context.heap().get_array(*array_id);
                let len = array.vec.len() as f64;

                context.push_value(Value::Float(len));

                Ok(())
            }))
            .with_method("push", Arc::new(|instance, context| {
                let Value::Array(array_id) = instance else {
                    unreachable!()
                };

                let value = context.pop_value();
                let array = context.heap().get_array_mut(*array_id);
                
                array.vec.push(value);

                context.push_value(Value::Void);

                Ok(())
            }))
            .with_method("reverse", Arc::new(|instance, context| {
                let Value::Array(array_id) = instance else {
                    unreachable!()
                };

                let array = context.heap().get_array_mut(*array_id);
                
                array.vec.reverse();

                context.push_value(Value::Void);

                Ok(())
            }))
            .with_method("join", Arc::new(|instance, context| {
                let Value::Array(array_id) = instance else {
                    unreachable!()
                };

                let string_id = context.pop_string_id();
                let separator = context.heap().get_string(string_id).clone();

                let vec = context.heap().get_array(*array_id).vec.clone();
                
                let string = vec.iter().map(|value| context.stringify(&value)).collect::<Vec<String>>().join(&separator);
                
                let string_id = context.heap().allocate_string(string);

                context.push_value(Value::String(string_id));

                Ok(())
            }))
            .with_method("contains", Arc::new(|instance, context| {
                let Value::Array(array_id) = instance else {
                    unreachable!()
                };
                
                let value = context.pop_value();
                let array = context.heap().get_array_mut(*array_id);

                let result = Value::Bool(array.vec.contains(&value));

                context.push_value(result);

                Ok(())
            }));
        prototype
    };

    pub static ref STRING_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("String");
        prototype
            .with_method("toString", Arc::new(|instance, context| {
                let string = ValueUtils::to_string(instance, context.heap());

                let string_id = context.heap().allocate_string(string);

                context.push_value(Value::String(string_id));

                Ok(())
            }));
        prototype
    };

    pub static ref NUMBER_PROTOTYPE: BuiltInPrototype = {
        let mut prototype = BuiltInPrototype::new("Number");
        prototype
            .with_method("toString", Arc::new(|instance, context| {
                let string = ValueUtils::to_string(instance, context.heap());

                let string_id = context.heap().allocate_string(string);

                context.push_value(Value::String(string_id));

                Ok(())
            }));
        prototype
    };
}

// lazy_static! {
//     pub static ref OBJECT_PROTOTYPE: BuiltInPrototype = {
//         let mut prototype = BuiltInPrototype::new("Object");
//         prototype
//             .with_method("toString", Arc::new(|instance, args, _| {
//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 Ok(Value::String(instance.to_string()))
//             }));
//         prototype
//     };

//     pub static ref ARRAY_PROTOTYPE: BuiltInPrototype = {
//         let mut prototype = BuiltInPrototype::new("Array");
//         prototype
//             .with_method("toString", Arc::new(|instance, args, _| {
//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 Ok(Value::String(instance.to_string()))
//             }))
//             .with_method("len", Arc::new(|instance, args, _context| {
//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };

//                 Ok(Value::Number(array.fetch().len() as f64))
//             }))
//             .with_method("push", Arc::new(|instance, args, _context| {
//                 if args.len() > 1 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };

//                 array.push(args[0].clone());

//                 Ok(Value::Void)
//             }))
//             .with_method("reverse", Arc::new(|instance, args, _context| {
//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };

//                 array.reverse();

//                 Ok(Value::Void)
//             }))
//             .with_method("join", Arc::new(|instance, args, _context| {
//                 let Some(Value::String(separator)) = args.get(0) else {
//                     return Err(LanguageError::from(RuntimeError::Custom("Expected string at index 0".into())));
//                 };

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };
                
//                 Ok(Value::String(array.fetch().iter().map(|arg| arg.to_string()).collect::<Vec<String>>().join(separator)))
//             }))
//             .with_method("find", Arc::new(|instance, args, context| {
//                 let Some(Value::Fn(predicate)) = args.get(0) else {
//                     return Err(LanguageError::from(RuntimeError::Custom("Expected string at index 0".into())));
//                 };

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };

//                 for value in array.fetch() {
//                     let result = match predicate {
//                         crate::parser::FunctionKind::BuiltIn(predicate) => {
//                             predicate.func.call(instance, &[value.clone()], context)?
//                         }
//                         crate::parser::FunctionKind::Runtime(predicate) => {
//                             eval_runtime_function(predicate.clone(), &[value.clone()], context)?
//                         }
//                     };

//                     if let Value::Bool(true) = result {
//                         return Ok(value)
//                     }
//                 }

//                 Ok(Value::Void)
//             }))
//             .with_method("contains", Arc::new(|instance, args, _context| {
//                 let Some(value) = args.get(0) else {
//                     return Err(LanguageError::from(RuntimeError::Custom("Expected value at index 0".into())));
//                 };

//                 let Value::Array(array) = instance else {
//                     unreachable!()
//                 };
                
//                 Ok(Value::Bool(array.fetch().contains(value)))
//             }));
//         prototype
//     };

//     pub static ref STRING_PROTOTYPE: BuiltInPrototype = {
//         let mut prototype = BuiltInPrototype::new("String");
//         prototype
//             .with_operator(Operation::Addition, Arc::new(|a, b| {
//                 let Value::String(value) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected string".to_string())))
//                 };

//                 let mut value = value.to_owned();

//                 value.push_str(&b.to_string());
        
//                 Ok(Value::String(value))
//             }))
//             .with_method("upper", Arc::new(|instance, args, _| {
//                 let Value::String(string) = instance else {
//                     unreachable!()
//                 };

//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 Ok(Value::String(string.to_uppercase()))
//             }))
//             .with_method("lower", Arc::new(|instance, args, _| {
//                 let Value::String(string) = instance else {
//                     unreachable!()
//                 };

//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 Ok(Value::String(string.to_lowercase()))
//             }))
//             .with_method("split", Arc::new(|instance, args, _| {
//                 let Some(Value::String(separator)) = args.get(0) else {
//                     return Err(LanguageError::from(RuntimeError::Custom("Expected string at index 0".into())));
//                 };

//                 let Value::String(string) = instance else {
//                     unreachable!()
//                 };

//                 Ok(Value::Array(ArrayRef::new(string.split(separator).map(|item| Value::String(item.to_string())).collect())))
//             }));
//         prototype
//     };

//     pub static ref NUMBER_PROTOTYPE: BuiltInPrototype = {
//         let mut prototype = BuiltInPrototype::new("Number");
//         prototype
//             .with_method("toString", Arc::new(|instance, args, _| {
//                 if args.len() > 0 {
//                     return Err(LanguageError::from(RuntimeError::Custom("Too many arguments".into())));
//                 }

//                 Ok(Value::String(instance.to_string()))
//             }))
//             .with_operator(Operation::Addition, Arc::new(|a, b| {
//                 let Value::Number(a) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };

//                 let Value::Number(b) = b else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };
        
//                 Ok(Value::Number(*a + *b))
//             }))
//             .with_operator(Operation::Substraction, Arc::new(|a, b| {
//                 let Value::Number(a) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };

//                 let Value::Number(b) = b else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };
        
//                 Ok(Value::Number(*a - *b))
//             }))
//             .with_operator(Operation::Multiplication, Arc::new(|a, b| {
//                 let Value::Number(a) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };

//                 let Value::Number(b) = b else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };
        
//                 Ok(Value::Number(*a * *b))
//             }))
//             .with_operator(Operation::Division, Arc::new(|a, b| {
//                 let Value::Number(a) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };

//                 let Value::Number(b) = b else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };
        
//                 Ok(Value::Number(*a / *b))
//             }))
//             .with_operator(Operation::Modulo, Arc::new(|a, b| {
//                 let Value::Number(a) = a else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };

//                 let Value::Number(b) = b else {
//                     return Err(LanguageError::from(RuntimeError::InvalidType("Expected number".to_string())))
//                 };
                
//                 Ok(Value::Number(*a % *b))
//             }));
//         prototype
//     };
// }