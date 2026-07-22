use std::collections::HashMap;

use crate::{compiler::{CompileInfo, Compiler}, vm::{Function, FunctionId, FunctionRef, ModuleData, Op, UpvalueDescriptor, UpvalueSource, Value}};

pub fn run_test_program() {
    let mut compiler = Compiler::new(CompileInfo::no_resolver());

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
    // compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Pop);
    
    // Define the variable that will be mutated
    compiler.string_constant("Hello, World!");
    // compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Return);
    compiler.end_function();

    // Mutator function
    compiler.begin_function(vec![UpvalueDescriptor { index: 0, source: UpvalueSource::Local }]);
    compiler.emit(Op::LoadUpvalue(0));
    // compiler.emit(Op::CallNative(print));
    compiler.emit(Op::Pop);
    compiler.emit(Op::LoadUpvalue(0));
    let one = compiler.constant(Value::Float(1.0));
    compiler.emit(Op::Const(one));
    compiler.emit(Op::Add);
    compiler.emit(Op::Return);
    compiler.end_function();

    let build = compiler.build();

    let _modules = vec![
        ModuleData {
            name: "main".to_string(),
            code: build.operations,
            constants: build.consts,
            strings: build.strings,
            functions: build.functions,
            exports: HashMap::new(),
            imports: vec![]
        },
        ModuleData {
            name: "lib".to_string(),
            code: vec![
                Op::Const(0),
                // Op::CallNative(print),
                Op::Halt,
                // An empty function
                Op::Return
            ],
            constants: vec![
                Value::Int(69420)
            ],
            strings: vec![],
            functions: vec![Function { module_id: None, argument_count: 0, local_count: 0, code_offset: 3, upvalue_descriptors: vec![] }],
            exports: HashMap::new(),
            imports: vec![]
        }
    ];

    // execute(modules, ExecutionInfo { local_count: 3 });
}

pub fn run_imports_test_program() {
    let _lib = {
        let mut compiler = Compiler::new(CompileInfo::no_resolver());

        compiler.begin_function(vec![]);
        compiler.emit(Op::Load(1));
        compiler.emit(Op::Load(2));
        compiler.emit(Op::Add);
        compiler.emit(Op::Return);
        let function_id = compiler.end_function();

        compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(function_id))));
        compiler.emit(Op::Store(0));

        let build = compiler.build();

        let mut exports = HashMap::new();

        exports.insert("add".to_string(), 0);

        ModuleData {
            name: "lib".to_string(),
            code: build.operations,
            constants: build.consts,
            strings: build.strings,
            exports,
            functions: build.functions,
            imports: vec![]
        }
    };

    let _main = {
        let mut compiler = Compiler::new(CompileInfo::no_resolver());

        let string_id = compiler.pre_allocate_string("lib".to_string());
        compiler.emit(Op::Import(string_id));
        let a = compiler.constant(Value::Float(12.0));
        let b = compiler.constant(Value::Float(4.0));
        compiler.emit(Op::Const(a));
        compiler.emit(Op::Const(b));
        compiler.emit(Op::Call(2));
        // compiler.emit(Op::CallNative(super::print));

        let build = compiler.build();

        ModuleData {
            name: "lib".to_string(),
            code: build.operations,
            constants: build.consts,
            strings: build.strings,
            exports: build.exports,
            functions: build.functions,
            imports: vec![]
        }
    };
    
    // let mut modules = HashMap::new();

    // let entry_point = EntryPoint {
    //     module: main,
    //     execution_info: ExecutionInfo { local_count: 1 }
    // };

    // modules.insert("main".to_string(), Module {
    //     build,
    // });

    // let loader = Rc::new_cyclic(|loader| {
    //     let vm = create_vm(entry_point, loader.clone());
        
    //     RefCell::new(Loader { vm: Rc::new(RefCell::new(vm)), modules })
    // });

    // LoaderRef(loader)

    // execute(vec![main, lib], ExecutionInfo { local_count: 1 });
}