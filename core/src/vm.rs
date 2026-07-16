use std::{cell::RefCell, collections::HashMap, fs, path::Path, rc::Rc, time::Instant};

use nlf_shared::indexmap::IndexMap;
use serde::Serialize;
use smallvec::SmallVec;

use crate::{compiler::{Compiler, compile_program}, errors::LanguageResult, lexer, loader, parser, new_translator, vm};

type UpvalueRef = Rc<RefCell<Value>>;

#[derive(Clone, Debug)]
pub enum Local {
    Value(Value),
    Upvalue(UpvalueRef)
}

pub type HeapIndex = usize;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Value {
    String(HeapIndex),
    Float(f64),
    Int(u64),
    Bool(bool),
    Closure(HeapIndex),
    Object(HeapIndex),
    Array(HeapIndex),
    Void
}

// TODO: should not order using function id
#[derive(Clone, Debug, PartialEq, PartialOrd, Copy)]
pub struct FunctionId(pub usize);

#[derive(Debug)]
pub enum FunctionRef {
    Id(FunctionId),
    Name { module: String, name: String }
}

// impl PartialEq for Value {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (Value::Float(a), Value::Float(b)) => a == b
//         }
//     }
// }

pub struct ValueStack(SmallVec<[Value; 4]>);

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

    fn pop_float(&mut self) -> f64 {
        let Value::Float(float) = self.pop() else {
            panic!("Value is not float")
        };

        float
    }

    fn pop_bool(&mut self) -> bool {
        let Value::Bool(bool) = self.pop() else {
            panic!("Value is not bool")
        };

        bool
    }
}

impl Value {
    fn to_string(&self, heap: &Heap) -> String {
        match self {
            Self::Float(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::String(string_id) => heap.strings[*string_id].to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Closure(_) => format!("fn() {{}}"),
            Self::Array(_) => format!("Array"),
            Self::Object(_) => format!("Object"),
            Self::Void => format!("Void")
        }
    }
}

#[derive(Debug)]
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
    CallNative(fn(&mut ValueStack, &Heap)),
    EQ,
    NEQ,
    LT,
    LTE,
    GT,
    GTE,
    And,
    Or,
    Increment(usize),
    Jump(usize),
    JumpIfFalse(usize),
    Call(usize),
    Return,
    MakeClosure(FunctionRef),
    BuildObject(usize),
    BuildArray(usize),
    ReadProperty,
    WriteProperty,
    Halt
}

pub struct Object {
    map: IndexMap<String, Value>
}

impl Object {
    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    #[inline(always)]
    pub fn set(&mut self, key: &str, value: Value) {
        self.map.insert(key.to_string(), value);
    }
}

pub struct Array {
    vec: Vec<Value>
}

impl Array {
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.vec.get(index)
    }

    #[inline(always)]
    pub fn set(&mut self, index: usize, value: Value) {
        self.vec[index] = value;
    }
}

pub struct Heap {
    pub strings: Vec<String>,
    pub objects: Vec<Object>,
    pub arrays: Vec<Array>,
    pub closures: Vec<Closure>
}

impl Heap {
    pub fn new() -> Self {
        Self { strings: vec![], objects: vec![], arrays: vec![], closures: vec![] }
    }

    #[inline(always)]
    pub fn allocate_string(&mut self, string: String) -> usize {
        let id = self.strings.len();
        self.strings.push(string);
        id
    }

    #[inline(always)]
    pub fn allocate_object(&mut self, object: Object) -> usize {
        let id = self.objects.len();
        self.objects.push(object);
        id
    }

    #[inline(always)]
    pub fn allocate_array(&mut self, array: Array) -> usize {
        let id = self.arrays.len();
        self.arrays.push(array);
        id
    }

    #[inline(always)]
    pub fn allocate_closure(&mut self, closure: Closure) -> usize {
        let id = self.closures.len();
        self.closures.push(closure);
        id
    }
}

struct VM {
    pub stack: ValueStack,
    pub frames: Vec<Frame>,
    pub modules: Vec<Module>,
    pub functions: Vec<Function>,
    pub heap: Heap
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Closure {
    function_id: usize,
    upvalues: Vec<UpvalueRef>
}

struct Frame {
    pub module_id: usize,
    pub ip: usize,
    pub locals: Vec<Local>,
    pub closure: Option<HeapIndex>
}

#[derive(Debug, Clone, Serialize)]
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
    pub module_id: usize,
    pub argument_count: usize,
    pub local_count: usize,
    pub code_offset: usize,
    pub upvalue_descriptors: Vec<UpvalueDescriptor>
}

#[derive(Debug)]
pub struct Module {
    name: String,
    code: Vec<Op>,
    constants: Vec<Value>
}

pub fn print(stack: &mut ValueStack, heap: &Heap) {
    let message = stack.pop();
    
    println!("{}", message.to_string(heap));

    stack.push(Value::Void);
}

fn run(vm: &mut VM) {
    loop {
        let frame = vm.frames.last_mut().expect("No frame found");

        let VM { stack, modules, .. } = vm;

        let module = &modules[frame.module_id];

        let Some(instruction) = module.code.get(frame.ip) else {
            break
        };

        frame.ip += 1;

        match instruction {
            Op::Const(id) => {
                stack.push(module.constants[*id]);
            }
            Op::Pop => {
                stack.pop();
            }
            Op::CallNative(func) => {
                func(stack, &vm.heap);
            }
            Op::Add => {
                let b = stack.pop_float();
                let a = stack.pop_float();
                
                stack.push(Value::Float(a + b));
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
                
                stack.push(Value::Float(a * b));
            }
            Op::Store(slot) => {
                let value = stack.pop();

                frame.locals[*slot] = Local::Value(value);
            }
            Op::StoreCaptured(slot) => {
                let value = stack.pop();

                let upvalue_ref = Rc::new(RefCell::new(value));

                frame.locals[*slot] = Local::Upvalue(upvalue_ref);
            }
            Op::Load(slot) => {
                stack.push(match &frame.locals[*slot] {
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
                continue;
            }
            Op::JumpIfFalse(position) => {
                if let Value::Bool(false) = stack.pop() {
                    frame.ip = *position;
                    continue;
                }
            }
            Op::EQ => {
                let b = stack.pop();
                let a = stack.pop();

                stack.push(Value::Bool(a == b));
            }
            Op::NEQ => {
                let b = stack.pop();
                let a = stack.pop();

                stack.push(Value::Bool(a != b));
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
            Op::And => {
                let b = stack.pop_bool();
                let a = stack.pop_bool();

                stack.push(Value::Bool(a && b));
            }
            Op::Or => {
                let b = stack.pop_bool();
                let a = stack.pop_bool();

                stack.push(Value::Bool(a || b));
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
            Op::Call(argument_count) => {
                let mut arguments = vec![];
                for _ in 0..*argument_count {
                    arguments.push(Local::Value(stack.pop()));
                }

                let Value::Closure(closure_id) = stack.pop() else {
                    panic!("Failed to retrieve function")
                };

                let closure = &vm.heap.closures[closure_id];

                let function = vm.functions[closure.function_id].clone();

                let mut call_locals = vec![Local::Value(Value::Closure(closure_id))];
                call_locals.append(&mut arguments);
                call_locals.resize(function.argument_count + function.local_count, Local::Value(Value::Void));
                vm.frames.push(Frame { module_id: function.module_id, ip: function.code_offset, locals: call_locals, closure: Some(closure_id) });
            }
            Op::Return => {
                vm.frames.pop();
            }
            Op::MakeClosure(reference) => {
                match reference {
                    FunctionRef::Id(fn_id) => {
                        let function = &vm.functions[fn_id.0];
                        
                        let mut upvalues = vec![];

                        for upvlaue_descriptor in function.upvalue_descriptors.iter() {
                            match upvlaue_descriptor.source {
                                UpvalueSource::Local => {
                                    let Local::Upvalue(upvalue_ref) = frame.locals[upvlaue_descriptor.index].clone() else {
                                        panic!("Local is not an upvalue")
                                    };

                                    upvalues.push(upvalue_ref);
                                }
                                UpvalueSource::Upvalue => {
                                    let closure_id = *frame.closure.as_ref().expect("Closure should be defined");

                                    let upvalue_ref = vm.heap.closures[closure_id].upvalues[upvlaue_descriptor.index].clone();

                                    upvalues.push(upvalue_ref);
                                }
                            }
                        }

                        let closure = Closure {
                            function_id: fn_id.0,
                            upvalues
                        };

                        let closure_id = vm.heap.allocate_closure(closure);

                        stack.push(Value::Closure(closure_id));
                    }
                    _ => {}
                }
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
            Op::ReadProperty => {
                let property = stack.pop();
                let object = stack.pop();

                match object {
                    Value::Object(id) => {
                        let Value::String(string_id) = property else {
                            panic!("An object should only be indexed using a string")
                        };

                        let property = &vm.heap.strings[string_id];

                        stack.push(*vm.heap.objects[id].get(property).expect("Property doesn't exist"));
                    }
                    Value::Array(id) => {
                        let Value::Float(index) = property else {
                            panic!("An object should only be indexed using an int")
                        };

                        stack.push(*vm.heap.arrays[id].get(index as usize).expect("Index out of range"));
                    }
                    _ => panic!("Value not indexable")
                }
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
                            panic!("An object should only be indexed using an int")
                        };

                        vm.heap.arrays[id].set(index as usize, value);
                    }
                    _ => panic!("Value not indexable")
                }
            }
            Op::Halt => {
                break;
            }
        }
    }
}

pub struct ExecutionInfo {
    pub functions: Vec<Function>,
    pub heap: Heap,
    pub local_count: usize
}

pub fn execute(modules: Vec<Module>, excution_info: ExecutionInfo) {
    let mut locals = Vec::new();
    locals.resize(excution_info.local_count, Local::Value(Value::Void));

    let frames = vec![
        Frame {
            module_id: 0,
            ip: 0,
            locals,
            closure: None
        }
    ];

    let mut vm = VM {
        stack: ValueStack::new(),
        frames,
        modules,
        functions: excution_info.functions,
        heap: excution_info.heap
    };

    let start = Instant::now();
    run(&mut vm);
    let end = start.elapsed();
    println!("{end:.2?}");
    
    assert!(vm.stack.0.len() == 0, "Stack is not empty")
}

pub fn test_compile(path: &str) -> LanguageResult<()> {
    let module_source = loader::resolve_module(path, None)?;

    let contents = loader::read_module(&module_source)?;

    let tokens = lexer::lex(contents)?;
    
    let name = Path::new(&path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    
    let ast = parser::parse(tokens)?;

    let ir = new_translator::translate(ast)?;
    
    let local_count = ir.local_count; 
    
    fs::write("core/debug/vm/ir.ron", ron::ser::to_string_pretty(&ir.program, Default::default()).unwrap()).unwrap();

    let build = compile_program(ir);

    fs::write("core/debug/vm/build.dbg", build.operations.iter().map(|op| format!("{:?}", op)).collect::<Vec<String>>().join("\n")).expect("Failed to write operations debug file");
    fs::write("core/debug/vm/constants.dbg", format!("{:#?}", build.consts)).unwrap();

    let module = Module {
        code: build.operations,
        constants: build.consts,
        name: name.to_string()
    };

    vm::execute(vec![module], ExecutionInfo { functions: build.functions, heap: build.heap, local_count });

    Ok(())
}

pub fn test() {
    let mut compiler = Compiler::new(HashMap::new(), 0);

    let zero = compiler.constant(Value::Int(0));

    // Create loop variable
    compiler.emit(Op::Const(zero));
    compiler.emit(Op::Store(0));

    compiler.label("loop_body");

    // Increment loop variable by 1
    compiler.emit(Op::Increment(0));

    // Check if variable is equal to 1 million
    let one_million = compiler.constant(Value::Int(1_000_000));
    compiler.emit(Op::Load(0));
    compiler.emit(Op::Const(one_million));
    compiler.emit(Op::NEQ);

    // Jump to loop end if condition is met
    compiler.jump_if_false("loop_end");

    // Jump to loop body
    compiler.jump("loop_body");

    compiler.label("loop_end");

    // Print loop variable
    compiler.emit(Op::Load(0));
    compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Pop);
    
    // Define the variable that will be mutated
    let zero = compiler.constant(Value::Float(0.0));
    compiler.emit(Op::Const(zero));
    compiler.emit(Op::StoreCaptured(0));

    // Store the mutator closure
    compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(1))));
    compiler.emit(Op::Store(1));

    // Run the mutator function
    compiler.emit(Op::Load(1));
    compiler.emit(Op::Call(0));
    compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Pop);

    // Call the hello world function twice
    compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(0))));
    compiler.emit(Op::Store(2));
    for _ in 0..2 {
        compiler.emit(Op::Load(2));
        compiler.emit(Op::Call(0));
        compiler.emit(Op::Pop);
    }

    // End the program
    compiler.emit(Op::Halt);

    // Hello world function
    compiler.begin_function(vec![]);
    let hello_world = compiler.string_constant("Hello, World!");
    compiler.emit(Op::Const(hello_world));
    compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Return);
    compiler.end_function();

    // Mutator function
    compiler.begin_function(vec![UpvalueDescriptor { index: 0, source: UpvalueSource::Local }]);
    compiler.emit(Op::LoadUpvalue(0));
    compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Pop);
    compiler.emit(Op::LoadUpvalue(0));
    let one = compiler.constant(Value::Float(1.0));
    compiler.emit(Op::Const(one));
    compiler.emit(Op::Add);
    compiler.emit(Op::Return);
    compiler.end_function();

    let build = compiler.build();

    let modules = vec![
        Module {
            name: "main".to_string(),
            code: build.operations,
            constants: build.consts
        },
        Module {
            name: "lib".to_string(),
            code: vec![
                Op::Const(0),
                Op::CallNative(print),
                Op::Halt,
                // An empty function
                Op::Return
            ],
            constants: vec![
                Value::Int(69420)
            ]
        }
    ];

    let mut functions = build.functions;

    functions.push(Function { module_id: 1, argument_count: 0, local_count: 0, code_offset: 3, upvalue_descriptors: vec![] });
    
    execute(modules, ExecutionInfo { functions, heap: build.heap, local_count: 3 });
}