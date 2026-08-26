use std::{cell::RefCell, collections::{HashMap, HashSet}, ptr::NonNull, rc::{Rc, Weak}, time::Instant};

use nlf_shared::indexmap::IndexMap;
use serde::Serialize;
use smallvec::SmallVec;
use inline_colorization::*;

use crate::{compiler::{Import, ImportKind}, errors::{LanguageResult, RuntimeError}, loader::{EntryPoint, Loader, LoaderRef}, parser::VariableKind, stdlib::{self, NativeFunctionType}, vm::{format::FormatOptions, prototypes::{ARRAY_PROTOTYPE, Method, NUMBER_PROTOTYPE, OBJECT_PROTOTYPE, Operation, Prototype, STRING_PROTOTYPE}}};

pub use nlf_shared::vm::*;

pub mod programs;
pub mod format;
pub mod prototypes;

type UpvalueRef = Rc<RefCell<Value>>;

#[derive(Clone, Debug)]
pub enum Local {
    Value(Value),
    Upvalue(UpvalueRef)
}

// TODO: should not order using function id
#[derive(Clone, Debug, PartialEq, PartialOrd, Copy)]
pub struct FunctionId(pub usize);

#[derive(Debug, Clone)]
pub enum FunctionRef {
    Id(FunctionId),
    Name { module: String, name: String },
    Import(usize)
}

pub struct ValueStack(SmallVec<[Value; 8]>);

impl ValueStack {
    fn new() -> Self {
        Self(SmallVec::new())
    }

    fn push(&mut self, value: Value) {
        let _ = self.0.push(value);
    }

    fn pop(&mut self) -> Value {
        self.0.pop().expect("[StackUnderflow] Failed to retrieve value")
    }

    #[inline(always)]
    fn pop_float(&mut self) -> f64 {
        let Value::Float(float) = self.pop() else {
            panic!("Value is not float")
        };

        float
    }

    #[inline(always)]
    fn pop_bool(&mut self) -> bool {
        let Value::Bool(bool) = self.pop() else {
            panic!("Value is not bool")
        };

        bool
    }

    #[inline(always)]
    fn pop_closure(&mut self) -> HeapIndex {
        let Value::Closure(closure_id) = self.pop() else {
            panic!("Value is not closure")
        };

        closure_id
    }
}

struct ValueUtils;

impl ValueUtils {
    fn to_string(value: &Value, heap: &dyn AbstractHeap) -> String {
        match value {
            Value::String(string_id) => format!("{}", heap.get_string(*string_id)),
            Value::Float(value) => format!("{value}"),
            Value::Int(value) => format!("{value}"),
            Value::Bool(value) => format!("{value}"),
            Value::Closure(_) => format!("fn() {{ TODO }}"),
            Value::Native(_) => format!("fn() {{ native code }}"),
            Value::Object(object_id) => {
                let object = &heap.get_object(*object_id).map;
                format::format_object(object, heap, FormatOptions {
                    space: if object.len() > 1 { Some(2) } else { None }
                })
            },
            Value::Array(array_id) => {
                let array = &heap.get_array(*array_id).vec;

                format::format_array(array, heap, FormatOptions {
                    space: if array.len() > 1 { Some(2) } else { None }
                })
            },
            Value::Class(_) => format!("class {{}}"),
            Value::Method(_) => format!("fn {{ method code }}"),
            Value::Void => format!("Void"),
        }
    }

    fn to_string_pretty(value: &Value, heap: &dyn AbstractHeap) -> String {
        match value {
            Value::String(_) => Self::to_string(value, heap),
            Value::Float(_) => format!("{color_yellow}{}{color_reset}", Self::to_string(value, heap)),
            Value::Int(_) => format!("{color_yellow}{}{color_reset}", Self::to_string(value, heap)),
            Value::Bool(_) => format!("{color_blue}{}{color_reset}", Self::to_string(value, heap)),
            Value::Closure(_) => format!("{color_black}{}{color_reset}", Self::to_string(value, heap)),
            Value::Native(_) => format!("{color_black}{}{color_reset}", Self::to_string(value, heap)),
            Value::Object(_) => Self::to_string(value, heap),
            Value::Array(_) => Self::to_string(value, heap),
            Value::Class(_) => Self::to_string(value, heap),
            Value::Method(_) => Self::to_string(value, heap),
            Value::Void => Self::to_string(value, heap),
        }
    }

    fn get_prototype<'a>(value: &'a Value, _heap: &Heap) -> &'a dyn Prototype {
        match value {
            Value::Object(_) => &*OBJECT_PROTOTYPE,
            Value::Array(_) => &*ARRAY_PROTOTYPE,
            Value::String(_) => &*STRING_PROTOTYPE,
            Value::Float(_) => &*NUMBER_PROTOTYPE,
            _ => todo!()
        }
    }
}

#[derive(Debug, Clone)]
pub enum Op {
    Const(usize),
    Pop,
    Store(usize),
    StoreCaptured(usize),
    Load(usize),
    StoreUpvalue(usize),
    LoadUpvalue(usize),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    EQ,
    NEQ,
    LT,
    LTE,
    GT,
    GTE,
    Not,
    Increment(usize),
    Length,
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    Call(usize),
    Return,
    MakeClosure(FunctionRef),
    BindSelf,
    BuildString(usize),
    BuildObject(usize),
    BuildArray(usize),
    BuildClass,
    ReadProperty,
    WriteProperty,
    Import(HeapIndex),
    LoadNative(usize),
    Dup,
    Halt
}

#[derive(Clone, Default)]
pub struct Heap {
    pub strings: Vec<String>,
    pub objects: Vec<Object>,
    pub arrays: Vec<Array>,
    pub closures: Vec<Closure>,
    pub classes: Vec<Class>,
    pub native_functions: Vec<NativeFunctionType>,
    pub methods: Vec<Method>
}

impl Heap {
    #[inline(always)]
    pub fn allocate_closure(&mut self, closure: Closure) -> usize {
        let id = self.closures.len();
        self.closures.push(closure);
        id
    }

    #[inline(always)]
    pub fn allocate_method(&mut self, method: Method) -> usize {
        let id = self.methods.len();
        self.methods.push(method);
        id
    }

    #[inline]
    pub fn check_equality(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::String(string_a_id), Value::String(string_b_id)) => {
                let string_a = &self.strings[*string_a_id];
                let string_b = &self.strings[*string_b_id];
                
                string_a == string_b
            }
            (Value::Object(object_a_id), Value::Object(object_b_id)) => {
                let object_a = &self.objects[*object_a_id].map;
                let object_b = &self.objects[*object_b_id].map;

                if object_a.len() != object_b.len() {
                    return false
                }

                for ((key_a, value_a), (key_b, value_b)) in object_a.iter().zip(object_b) {
                    if key_a != key_b {
                        return false
                    }

                    if !self.check_equality(value_a, value_b) {
                        return false
                    }
                }

                true
            }
            (Value::Array(array_a_id), Value::Array(array_b_id)) => {
                let array_a = &self.arrays[*array_a_id].vec;
                let array_b = &self.arrays[*array_b_id].vec;

                if array_a.len() != array_b.len() {
                    return false
                }

                for (value_a, value_b) in array_a.iter().zip(array_b) {
                    if !self.check_equality(value_a, value_b) {
                        return false
                    }
                }

                true
            }
            _ => a == b
        }
    }

    pub fn get_size(&self) -> usize {
        // TODO: Count size recursively

        let mut total = 0;

        total += self.arrays.len() * size_of::<Array>();
        total += self.classes.len() * size_of::<Class>();
        total += self.closures.len() * size_of::<Closure>();
        total += self.methods.len() * size_of::<Method>();
        total += self.native_functions.len() * size_of::<NativeFunctionType>();
        total += self.objects.len() * size_of::<Object>();
        total += self.strings.len() * size_of::<String>();

        total
    }
}

impl AbstractHeap for Heap {
    #[inline(always)]
    fn allocate_string(&mut self, string: String) -> usize {
        let id = self.strings.len();
        self.strings.push(string);
        id
    }

    #[inline(always)]
    fn allocate_object(&mut self, object: Object) -> usize {
        let id = self.objects.len();
        self.objects.push(object);
        id
    }

    #[inline(always)]
    fn allocate_array(&mut self, array: Array) -> usize {
        let id = self.arrays.len();
        self.arrays.push(array);
        id
    }

    #[inline(always)]
    fn allocate_class(&mut self, class: Class) -> usize {
        let id = self.classes.len();
        self.classes.push(class);
        id
    }

    #[inline(always)]
    fn allocate_native_function(&mut self, native_function: NativeFunctionType) -> usize {
        let id = self.native_functions.len();
        self.native_functions.push(native_function);
        id
    }

    #[inline(always)]
    fn get_string(&self, string_id: usize) -> &String {
        &self.strings[string_id]
    }

    #[inline(always)]
    fn get_object(&self, object_id: usize) -> &Object {
        &self.objects[object_id]
    }

    #[inline(always)]
    fn get_array(&self, array_id: usize) -> &Array {
        &self.arrays[array_id]
    }

    #[inline(always)]
    fn get_array_mut(&mut self, array_id: usize) -> &mut Array {
        &mut self.arrays[array_id]
    }

    #[inline(always)]
    fn get_class(&self, class_id: usize) -> &Class {
        &self.classes[class_id]
    }

    #[inline(always)]
    fn get_native_function(&self, native_function_id: usize) -> NativeFunctionType {
        self.native_functions[native_function_id]
    }
}

pub struct VM {
    pub stack: ValueStack,
    pub frames: Vec<Frame>,
    pub modules: Vec<ModuleData>,
    pub heap: Heap,
    pub loader: Weak<RefCell<Loader>>
}

impl VM {
    pub fn run_module(&mut self, name: &str, import: Import) {
        let loader = self.loader.upgrade().expect("Failed to upgrade loader");

        let mut loader = loader.borrow_mut();

        let module = loader.get_or_load(name);

        let module_id = self.modules.len();

        let build = module.build.clone();

        self.modules.push(ModuleData {
            name: module.source.path.clone(),
            code: build.operations,
            constants: build.consts,
            strings: build.strings,
            functions: build.functions,
            exports: build.exports,
            imports: build.imports
        });

        let mut locals = vec![];
        locals.resize(module.ir.local_count, Local::Value(Value::Void));

        self.frames.push(Frame { module_id, ip: 0, locals, closure: None, import: Some(Box::new(import)) });
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Closure {
    module_id: usize,
    function_id: usize,
    upvalues: Vec<UpvalueRef>,
    self_value: Value
}

pub struct Frame {
    pub module_id: usize,
    pub ip: usize,
    pub locals: Vec<Local>,
    pub closure: Option<HeapIndex>,
    pub import: Option<Box<Import>>
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum UpvalueSource {
    Local,
    Upvalue
}

#[derive(Debug, Clone, Serialize)]
pub struct UpvalueDescriptor {
    pub source: UpvalueSource,
    pub index: usize
}

#[derive(Debug, Clone)]
pub struct Function {
    pub module_id: Option<usize>,
    pub argument_count: usize,
    pub local_count: usize,
    pub code_offset: usize,
    pub upvalue_descriptors: Vec<UpvalueDescriptor>,
    pub locals: Vec<VariableKind>
}

#[derive(Debug)]
pub struct ModuleData {
    pub name: String,
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    pub strings: Vec<String>,
    pub functions: Vec<Function>,
    pub exports: HashMap<String, usize>,
    pub imports: Vec<Import>
}

pub fn print(context: &mut dyn AbstractVMContext) {
    let message = context.pop_value();
    
    println!("{}", context.stringify(&message));

    context.push_value(Value::Void);
}


pub fn run(vm: &mut VM) {
    loop {
        if step(vm) {
            break;
        }
    }
}

#[inline(always)]
pub fn step(vm: &mut VM) -> bool {
    let vm_ptr: *mut VM = vm;

    let frame = vm.frames.last_mut().expect("No frame found");

    let VM { stack, modules, .. } = vm;

    let module = unsafe { modules.get_unchecked(frame.module_id) };

    let Some(instruction) = module.code.get(frame.ip) else {
        return true
    };

    frame.ip += 1;

    match instruction {
        Op::Const(id) => {
            stack.push(module.constants[*id]);
        }
        Op::Pop => {
            stack.pop();
        }
        Op::Add => {
            let b = stack.pop();
            let a = stack.pop();

            let prototype = ValueUtils::get_prototype(&a, &vm.heap);

            let mut context = VMContext {
                vm: NonNull::new(vm_ptr).unwrap()
            };
            
            stack.push(prototype.operate(Operation::Addition, &a, &b, &mut context).expect("Addition failed"));
        }
        Op::Sub => {
            let b = stack.pop_float();
            let a = stack.pop_float();
            
            stack.push(Value::Float(a - b));
        }
        Op::Mul => {
            let b = stack.pop_float();
            let a = stack.pop_float();
            
            stack.push(Value::Float(a * b));
        }
        Op::Div => {
            let b = stack.pop_float();
            let a = stack.pop_float();
            
            stack.push(Value::Float(a / b));
        }
        Op::Mod => {
            let b = stack.pop_float();
            let a = stack.pop_float();
            
            stack.push(Value::Float(a % b));
        }
        Op::Neg => {
            let a = stack.pop_float();

            stack.push(Value::Float(-a));
        }
        Op::Store(slot) => {
            let value = stack.pop();

            unsafe {
                *frame.locals.get_unchecked_mut(*slot) = Local::Value(value)
            };
        }
        Op::StoreCaptured(slot) => {
            let value = stack.pop();

            let upvalue_ref = Rc::new(RefCell::new(value));

            unsafe {
                *frame.locals.get_unchecked_mut(*slot) = Local::Upvalue(upvalue_ref)
            };
        }
        Op::Load(slot) => {
            stack.push(match unsafe { frame.locals.get_unchecked(*slot) } {
                Local::Value(value) => *value,
                Local::Upvalue(upvalue_ref) => upvalue_ref.borrow().clone()
            });
        }
        Op::StoreUpvalue(slot) => {
            let value = stack.pop();

            let closure_id = *frame.closure.as_ref().expect("StoreUpvalue in non-closure");

            let closure = &vm.heap.closures[closure_id];

            *closure.upvalues[*slot].borrow_mut() = value;

        }
        Op::LoadUpvalue(slot) => {
            let closure_id = *frame.closure.as_ref().expect("LoadUpvalue in non-closure");

            let closure = &vm.heap.closures[closure_id];

            stack.push(closure.upvalues[*slot].borrow().clone());
        }
        Op::Jump(position) => {
            frame.ip = *position;
            return false;
        }
        Op::JumpIfFalse(position) => {
            if let Value::Bool(false) | Value::Void = stack.pop() {
                frame.ip = *position;
                return false;
            }
        }
        Op::JumpIfTrue(position) => {
            if !matches!(stack.pop(), Value::Bool(false) | Value::Void) {
                frame.ip = *position;
                return false;
            }
        }
        Op::EQ => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(vm.heap.check_equality(&a, &b)));
        }
        Op::NEQ => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(!vm.heap.check_equality(&a, &b)));
        }
        Op::LT => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(a < b));
        }
        Op::LTE => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(a <= b));
        }
        Op::GT => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(a > b));
        }
        Op::GTE => {
            let b = stack.pop();
            let a = stack.pop();

            stack.push(Value::Bool(a >= b));
        }
        Op::Not => {
            let a = stack.pop_bool();

            stack.push(Value::Bool(!a));
        }
        Op::Increment(slot) => {
            let value = frame.locals.get_mut(*slot).expect("Local not found");

            let value = match value {
                Local::Value(value) => value,
                Local::Upvalue(value) => &mut value.borrow_mut()
            };

            match value {
                Value::Float(number) => {
                    *number += 1.0
                }
                Value::Int(number) => {
                    *number += 1
                }
                _ => panic!("Can only increment numbers")
            }
        }
        Op::Length => {
            let Value::Array(array_id) = stack.pop() else {
                panic!("Op::Length only works on arrays")
            };

            stack.push(Value::Float(vm.heap.get_array(array_id).vec.len() as f64));
        }
        Op::Call(argument_count) => {
            match stack.pop() {
                Value::Closure(closure_id) => {
                    let closure = &vm.heap.closures[closure_id];

                    let function_module = &modules[closure.module_id];

                    let function = &function_module.functions[closure.function_id];

                    // if function.argument_count != *argument_count  {
                    //     panic!("Expected {} argument(s), got {}", function.argument_count, argument_count)
                    // }

                    let create_argument = |index: usize, value: Value| -> Local {
                        match function.locals[index] {
                            VariableKind::Local => {
                                Local::Value(value)
                            }
                            VariableKind::Upvalue => {
                                Local::Upvalue(Rc::new(RefCell::new(value)))
                            }
                        }
                    };

                    let mut arguments = vec![];
                    for i in 0..*argument_count {
                        let local_index = i + 2;
                        arguments.push(create_argument(local_index, stack.pop()));
                    }

                    // 0: The called closure, 1: The closure self value
                    let mut call_locals = vec![create_argument(0, Value::Closure(closure_id)), create_argument(1, closure.self_value)];
                    call_locals.append(&mut arguments);
                    call_locals.resize(function.argument_count + function.local_count, Local::Value(Value::Void));

                    vm.frames.push(Frame { module_id: closure.module_id, ip: function.code_offset, locals: call_locals, closure: Some(closure_id), import: None });
                }
                Value::Native(function_id) => {
                    let mut context = VMContext {
                        vm: NonNull::new(vm_ptr).unwrap()
                    };

                    let global = vm.heap.native_functions[function_id];

                    global(&mut context).expect("An error occured inside a native function");
                }
                Value::Class(class_id) => {
                    let class = &vm.heap.classes[class_id];
                    
                    let closure = &vm.heap.closures[class.constructor_id];

                    let function_module = &modules[closure.module_id];

                    let function = &function_module.functions[closure.function_id];

                    let mut upvalue_locals = HashSet::new();

                    for descriptor in function.upvalue_descriptors.iter() {
                        upvalue_locals.insert(descriptor.index);
                    }

                    let mut arguments = vec![];
                    for i in 0..*argument_count {
                        let local_index = i + 2;
                        if upvalue_locals.contains(&local_index) {
                            arguments.push(Local::Value(stack.pop()));
                        } else {
                            arguments.push(Local::Upvalue(Rc::new(RefCell::new(stack.pop()))));
                        }
                    }

                    let mut call_locals = vec![Local::Value(Value::Void), Local::Value(closure.self_value)];
                    call_locals.append(&mut arguments);
                    call_locals.resize(function.argument_count + function.local_count, Local::Value(Value::Void));
                    vm.frames.push(Frame { module_id: closure.module_id, ip: function.code_offset, locals: call_locals, closure: Some(class.constructor_id), import: None });
                }
                Value::Method(method_id) => {
                    let method = &vm.heap.methods[method_id];

                    let mut context = VMContext {
                        vm: NonNull::new(vm_ptr).unwrap()
                    };

                    method.call(&stack.pop(), &mut context).expect("Method execution failed");
                }
                _ => panic!("Failed to retrieve function")
            };
        }
        Op::Return => {
            vm.frames.pop();
        }
        Op::MakeClosure(reference) => {
            println!("Heap size: {:.2} MB", vm.heap.get_size() as f64 / 1000000.0);
            match reference {
                FunctionRef::Id(fn_id) => {
                    let function = &module.functions[fn_id.0];
                    
                    let mut upvalues = vec![];

                    for upvalue_descriptor in function.upvalue_descriptors.iter() {
                        match upvalue_descriptor.source {
                            UpvalueSource::Local => {
                                let Local::Upvalue(upvalue_ref) = frame.locals[upvalue_descriptor.index].clone() else {
                                    panic!("Local is not an upvalue")
                                };

                                upvalues.push(upvalue_ref);
                            }
                            UpvalueSource::Upvalue => {
                                let closure_id = *frame.closure.as_ref().expect("Closure should be defined");

                                let upvalue_ref = vm.heap.closures[closure_id].upvalues[upvalue_descriptor.index].clone();

                                upvalues.push(upvalue_ref);
                            }
                        }
                    }

                    let closure = Closure {
                        module_id: frame.module_id,
                        function_id: fn_id.0,
                        upvalues,
                        self_value: Value::Void
                    };
                    
                    let closure_id = vm.heap.allocate_closure(closure);

                    stack.push(Value::Closure(closure_id));
                }
                _ => {}
            }
        }
        Op::BindSelf => {
            let self_value = stack.pop();
            let closure_id = stack.pop_closure();

            let closure = vm.heap.closures.get_mut(closure_id).expect("Closure not found");

            closure.self_value = self_value;

            stack.push(Value::Closure(closure_id));
        }
        Op::BuildString(string_id) => {
            let string = module.strings[*string_id].clone();

            let object_id = vm.heap.allocate_string(string);

            stack.push(Value::String(object_id));
        }
        Op::BuildObject(elements) => {
            let mut map = IndexMap::new();

            for _ in 0..*elements {
                let value = stack.pop();
                let key = stack.pop();

                let Value::String(string_id) = key else {
                    panic!()
                };

                let key = vm.heap.strings[string_id].clone();

                map.insert(key, value);
            }

            let object_id = vm.heap.allocate_object(Object { map });

            stack.push(Value::Object(object_id));
        }
        Op::BuildArray(elements) => {
            let mut vec = vec![];

            for _ in 0..*elements {
                let value = stack.pop();

                vec.push(value);
            }

            let object_id = vm.heap.allocate_array(Array { vec });

            stack.push(Value::Array(object_id));
        }
        Op::BuildClass => {
            let Value::Closure(constructor_id) = stack.pop() else {
                panic!("Tried to build a class using a non-closure constructor")
            };

            // let mut map: IndexMap<_, _> = IndexMap::new();

            // for _ in 0..*static_properties {
            //     let value = stack.pop();
            //     let key = stack.pop();

            //     let Value::String(string_id) = key else {
            //         panic!()
            //     };

            //     let key = vm.heap.strings[string_id].clone();

            //     map.insert(key, value);
            // }

            let class_id = vm.heap.allocate_class(Class {
                static_fields: IndexMap::new(),
                constructor_id
            });

            stack.push(Value::Class(class_id));
        }
        Op::ReadProperty => {
            let property = stack.pop();
            let object = stack.pop();

            match object {
                Value::Object(id) => {
                    if let Value::String(string_id) = property {
                        let property = &vm.heap.strings[string_id];

                        stack.push(*vm.heap.objects[id].get(property).expect("Property doesn't exist"));

                        return false;
                    };
                }
                Value::Array(id) => {
                    if let Value::Float(index) = property {
                        stack.push(*vm.heap.arrays[id].get(index as usize).expect("Index out of range"));
                        return false;
                    };
                }
                Value::Class(id) => {
                    if let Value::String(string_id) = property {
                        let property = &vm.heap.strings[string_id];

                        stack.push(*vm.heap.classes[id].static_fields.get(property).expect("Property doesn't exist"));
                        
                        return false;
                    };
                }
                _ => {}
            }

            let Value::String(string_id) = property else {
                panic!("An object should only be indexed using a string")
            };

            let property = &vm.heap.strings[string_id];

            let prototype = ValueUtils::get_prototype(&object, &vm.heap);

            let method = prototype.get_method(property).expect(&format!("Method \"{property}\" not found"));

            if let Some(Op::Call(_)) = module.code.get(frame.ip) {
                stack.push(object);
            }

            stack.push(Value::Method(vm.heap.allocate_method(method)));
        }
        Op::WriteProperty => {
            let value = stack.pop();
            let property = stack.pop();
            let object = stack.pop();

            match object {
                Value::Object(id) => {
                    let Value::String(string_id) = property else {
                        panic!("An object should only be indexed using a string")
                    };

                    let property = &vm.heap.strings[string_id];

                    vm.heap.objects[id].set(property, value);
                }
                Value::Array(id) => {
                    let Value::Float(index) = property else {
                        panic!("An array should only be indexed using an int")
                    };

                    vm.heap.arrays[id].set(index as usize, value);
                }
                Value::Class(id) => {
                    let Value::String(string_id) = property else {
                        panic!("A class should only be indexed using a string")
                    };

                    let property = &vm.heap.strings[string_id];

                    vm.heap.classes[id].static_fields.insert(property.clone(), value);
                }
                _ => panic!("Value not indexable")
            }
        }
        Op::Import(import_id) => {
            let import_id = *import_id;

            let import = module.imports[import_id].clone();

            let source = &import.source.clone();

            vm.run_module(source, import);
        }
        Op::LoadNative(string_id) => {
            let string = &module.strings[*string_id];

            let function_table = stdlib::FUNCTION_TABLE.lock();
            
            let global_id = function_table.iter().position(|(name, _)| name == string).expect(&format!("Global \"{}\" not found", string));

            stack.push(Value::Native(global_id));
        }
        Op::Dup => {
            stack.push(*stack.0.last().unwrap());
        }
        Op::Halt => {
            if frame.module_id == 0 {
                return true;
            }

            let import = frame.import.as_ref().unwrap();

            match &import.kind {
                ImportKind::Specific(names) => {
                    for name in names.iter().rev() {
                        let slot = module.exports.get(name).expect(&format!("Export \"{}\" not found", name));

                        let value = match &frame.locals[*slot] {
                            Local::Value(value) => *value,
                            Local::Upvalue(value) => *value.borrow()
                        };

                        stack.push(value);
                    }
                }
                _ => todo!()
            }

            vm.frames.pop();
        }
    }

    false
}

pub struct ExecutionInfo {
    pub local_count: usize
}

fn create_globals() -> Vec<NativeFunctionType> {
    let function_table = stdlib::FUNCTION_TABLE.lock();

    function_table.values().map(|function| *function).collect()
}

pub fn execute(loader: LoaderRef, modules: Vec<ModuleData>, excution_info: ExecutionInfo) {
    let mut locals = Vec::new();
    locals.resize(excution_info.local_count, Local::Value(Value::Void));

    let frames = vec![
        Frame {
            module_id: 0,
            ip: 0,
            locals,
            closure: None,
            import: None
        }
    ];
    
    let mut vm = VM {
        stack: ValueStack::new(),
        frames,
        modules,
        heap: Heap {
            native_functions: create_globals(),
            ..Default::default()
        },
        loader: Rc::downgrade(&loader.0)
    };

    if cfg!(not(target_arch = "wasm32")) {
        let start = Instant::now();
        run(&mut vm);
        let end = start.elapsed();
        println!("{end:.2?}");
    } else {
        run(&mut vm);
    }
    
    assert!(vm.stack.0.len() == 0, "Stack is not empty")
}

pub fn run_main(entry_point: EntryPoint) -> LanguageResult<()> {
    let _modules = vec![
        entry_point.module
    ];

    // execute(modules, entry_point.execution_info);

    Ok(())
}

pub fn create_vm(entry_point: EntryPoint, loader: Weak<RefCell<Loader>>) -> VM {
    let mut locals = Vec::new();
    locals.resize(entry_point.execution_info.local_count, Local::Value(Value::Void));

    let frames = vec![
        Frame {
            module_id: 0,
            ip: 0,
            locals,
            closure: None,
            import: None
        }
    ];

    let vm = VM {
        stack: ValueStack::new(),
        frames,
        modules: vec![entry_point.module],
        heap: Heap {
            native_functions: create_globals(),
            ..Default::default()
        },
        loader
    };

    vm
}

pub struct VMContext {
    vm: NonNull<VM> 
}

impl AbstractVMContext for VMContext {
    fn pop_value(&mut self) -> Value {
        unsafe {
            let vm = self.vm.as_mut();

            vm.stack.pop()
        }
    }

    fn pop_string_id(&mut self) -> usize {
        unsafe {
            let vm = self.vm.as_mut();

            let Value::String(string_id) = vm.stack.pop() else {
                panic!("Failed to retrieve string")
            };

            string_id
        }
    }

    fn get_string(&mut self, string_id: usize) -> &str {
        unsafe {
            let vm = self.vm.as_mut();

            &vm.heap.strings[string_id]
        }
    }
    
    fn pop_float(&mut self) -> f64 {
        unsafe {
            let vm = self.vm.as_mut();

            let Value::Float(float) = vm.stack.pop() else {
                panic!("Failed to retrieve float")
            };

            float
        }
    }

    fn pop_bool(&mut self) -> bool {
        unsafe {
            let vm = self.vm.as_mut();

            let Value::Bool(bool) = vm.stack.pop() else {
                panic!("Failed to retrieve string")
            };

            bool
        }
    }

    fn push_value(&mut self, value: Value) {
        unsafe {
            let vm = self.vm.as_mut();

            vm.stack.push(value);
        }
    }

    fn stringify(&mut self, value: &Value) -> String {
        unsafe {
            let vm = self.vm.as_mut();

            ValueUtils::to_string_pretty(value, &vm.heap)
        }
    }

    #[inline(always)]
    fn heap(&mut self) -> &mut dyn AbstractHeap {
        unsafe {
            &mut self.vm.as_mut().heap
        }
    }
    
    fn call(&mut self, callee: &Value, arguments: Vec<Value>) -> LanguageResult<Value> {
        let mut vm: &mut VM = unsafe { self.vm.as_mut() };

        match callee {
            Value::Closure(closure_id) => {
                let closure = &vm.heap.closures[*closure_id];

                let function_module = &vm.modules[closure.module_id];

                let function = &function_module.functions[closure.function_id];

                let mut call_locals = vec![Local::Value(Value::Closure(*closure_id)), Local::Value(closure.self_value)];
                call_locals.append(&mut arguments.iter().map(|value| Local::Value(*value)).collect());
                call_locals.resize(function.argument_count + function.local_count, Local::Value(Value::Void));

                let start_frame = vm.frames.len();

                vm.frames.push(Frame { module_id: closure.module_id, ip: function.code_offset, locals: call_locals, closure: Some(*closure_id), import: None });

                loop {
                    if step(&mut vm) {
                        break;
                    }

                    if start_frame == vm.frames.len() {
                        break;
                    }
                }

                Ok(vm.stack.pop())
            }
            _ => panic!("Only closure can be called from the VM context")
        }
    }
}

pub type RuntimeResult = Result<Value, RuntimeError>;