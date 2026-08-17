use std::{collections::HashMap, path::PathBuf, rc::Rc};

use serde::Serialize;

use crate::{compiler::resolvers::{FileSystemModuleResolver, ModuleResolver, NonImplementedModuleResolver, StaticModuleResolver}, lexer::TokenKind, translator::TranslatorOutput, parser::{ASTStatementKind, Expression, ExpressionKind, IRBlock, IRStatement, IRStatementKind, Iterable, Literal, LiteralExpressionKind, Statement, StatementKind, StatementKindWrapper, VariableKind, VariableRef}, vm::{Function, FunctionId, FunctionRef, Op, UpvalueDescriptor, Value}};

pub mod resolvers;

#[derive(Debug, Clone)]
pub enum ImportKind {
    Wildcard,
    Specific(Vec<String>)
}

#[derive(Debug, Clone)]
pub struct Import {
    pub kind: ImportKind,
    pub source: String
}

enum UnresolvedOp {
    Jump(String),
    JumpIfFalse(String),
    JumpIfTrue(String)
}

enum Instruction {
    Resolved(Op),
    Unresolved(UnresolvedOp)
}

#[derive(Debug)]
pub enum ValueUsage {
    Needed,
    Discard
}

pub struct AbstractFunction {
    instructions: Vec<Instruction>,
    completed: bool,
    upvalue_descriptors: Vec<UpvalueDescriptor>,
    labels: Vec<String>,
    argument_count: usize, 
    locals: Vec<VariableKind>
}

#[derive(Clone)]
pub struct CompilerBuild {
    pub operations: Vec<Op>,
    pub consts: Vec<Value>,
    pub functions: Vec<Function>,
    pub strings: Vec<String>,
    pub exports: HashMap<String, usize>,
    pub imports: Vec<Import>
}

pub struct Compiler {
    labels: HashMap<String, usize>,
    instructions: Vec<Instruction>,
    consts: Vec<Value>,
    functions: Vec<AbstractFunction>,
    label_id: usize,
    function_infos: FunctionInfos,
    class_infos: ClassInfos,
    strings: Vec<String>,
    loops: Vec<Loop>,
    source: PathBuf,
    path_resolver: Rc<dyn ModuleResolver>,
    exports: HashMap<String, usize>,
    imports: Vec<Import>
}

struct Loop {
    next_iteration_label: String,
    end_label: String
}

pub type FunctionInfos = HashMap<VariableRef, FunctionInfo>;
pub type ClassInfos = HashMap<VariableRef, ClassInfo>;

#[derive(Serialize, Debug, Clone)]
pub struct FunctionInfo {
    pub upvalue_descriptors: Vec<UpvalueDescriptor>,
    pub locals: Vec<VariableKind>
}

#[derive(Serialize, Debug, Clone)]
pub struct ClassInfo {
    pub methods: HashMap<String, FunctionInfo>
}

pub struct CompileInfo {
    pub source: PathBuf,
    pub function_infos: FunctionInfos,
    pub class_infos: ClassInfos,
    pub path_resolver: Rc<dyn ModuleResolver>
}

impl CompileInfo {
    pub fn module_resolver(process_path: PathBuf) -> Self {
        CompileInfo {
            source: "<module>".into(),
            function_infos: FunctionInfos::new(),
            class_infos: ClassInfos::new(),
            path_resolver: Rc::new(FileSystemModuleResolver {
                process_path
            })
        }
    }

    pub fn no_resolver() -> Self {
        CompileInfo {
            source: "<module>".into(),
            function_infos: FunctionInfos::new(),
            class_infos: ClassInfos::new(),
            path_resolver: Rc::new(NonImplementedModuleResolver)
        }
    }

    pub fn basic_resolver(modules: HashMap<String, String>) -> Self {
        CompileInfo {
            source: "<module>".into(),
            function_infos: FunctionInfos::new(),
            class_infos: ClassInfos::new(),
            path_resolver: Rc::new(StaticModuleResolver {
                modules
            })
        }
    }
}

impl Compiler {
    pub fn new(compile_info: CompileInfo) -> Self {
        Self {
            source: compile_info.source,
            labels: HashMap::new(),
            instructions: vec![],
            consts: vec![],
            functions: vec![],
            label_id: 0,
            function_infos: compile_info.function_infos,
            class_infos: compile_info.class_infos,
            strings: vec![],
            loops: vec![],
            path_resolver: compile_info.path_resolver,
            exports: HashMap::new(),
            imports: vec![]
        }
    }

    pub fn label(&mut self, name: &str) {
        for (_, function) in self.functions.iter_mut().rev().enumerate() {
            if function.completed {
                continue
            }

            function.labels.push(name.to_string());
            self.labels.insert(name.to_string(), function.instructions.len());

            return;
        }

        self.labels.insert(name.to_string(), self.instructions.len());
    }

    pub fn generate_label(&mut self) -> String {
        let name = self.label_id.to_string();
        self.label_id += 1;
        name
    }

    fn emit_instruction(&mut self, instruction: Instruction) {
        for function in self.functions.iter_mut().rev() {
            if function.completed {
                continue
            }

            function.instructions.push(instruction);

            return
        }
        self.instructions.push(instruction);
    }

    pub fn emit(&mut self, op: Op) {
        self.emit_instruction(Instruction::Resolved(op));
    }

    pub fn jump(&mut self, label: &str) {
        self.emit_instruction(Instruction::Unresolved(UnresolvedOp::Jump(label.to_string())));
    }
    
    pub fn jump_if_false(&mut self, label: &str) {
        self.emit_instruction(Instruction::Unresolved(UnresolvedOp::JumpIfFalse(label.to_string())));
    }

    pub fn jump_if_true(&mut self, label: &str) {
        self.emit_instruction(Instruction::Unresolved(UnresolvedOp::JumpIfTrue(label.to_string())));
    }

    pub fn begin_function_with(&mut self, abstract_function: AbstractFunction) {
        self.functions.push(abstract_function);
    }

    pub fn begin_function(&mut self, upvalue_descriptors: Vec<UpvalueDescriptor>, locals: Vec<VariableKind>) {
        self.begin_function_with(AbstractFunction { instructions: vec![], completed: false, upvalue_descriptors, labels: vec![], argument_count: 0, locals });
    }

    pub fn end_function(&mut self) -> usize {
        for (index, function) in self.functions.iter_mut().rev().enumerate() {
            if function.completed {
                continue
            }

            function.completed = true;

            return self.functions.len() - index - 1
        }
        
        unreachable!()
    }

    pub fn register_import(&mut self, import: Import) -> usize {
        let id = self.imports.len();
        self.imports.push(import);
        id
    }

    /// Pre allocates a string and returns the string id
    pub fn pre_allocate_string(&mut self, string: String) -> usize {
        for (index, allocated_string) in self.strings.iter().enumerate() {
            if *allocated_string == string {
                return index
            }
        }

        let string_id = self.strings.len();
        self.strings.push(string);
        string_id
    }

    pub fn string_constant(&mut self, string: &str) {
        let string_id = self.pre_allocate_string(string.to_string());

        self.emit(Op::BuildString(string_id));
    }

    pub fn constant(&mut self, value: Value) -> usize {
        for (i, constant) in self.consts.iter().enumerate() {
            if value == *constant {
                return i
            }
        }

        self.consts.push(value);

        self.consts.len() - 1
    }

    fn resolve_label(labels: &HashMap<String, usize>, label: &str) -> usize {
        *labels.get(label).expect(format!("Label \"{}\" not found", label).as_str())
    }

    pub fn build(mut self) -> CompilerBuild {
        let mut labels = self.labels;

        let mut functions: Vec<Function> = vec![];

        for (_, function) in self.functions.iter_mut().enumerate() {
            for label in &function.labels {
                *labels.get_mut(label).expect("Label not found") += self.instructions.len();
            }

            functions.push(Function {
                module_id: None,
                argument_count: function.argument_count,
                local_count: function.locals.len(),
                code_offset: self.instructions.len(),
                upvalue_descriptors: function.upvalue_descriptors.clone(),
                locals: function.locals.clone()
            });

            self.instructions.append(&mut function.instructions);
        }

        let operations: Vec<Op> = self.instructions.into_iter().map(|instruction| {
            match instruction {
                Instruction::Resolved(op) => op,
                Instruction::Unresolved(op) => {
                    match op {
                        UnresolvedOp::Jump(label) => {
                            Op::Jump(Self::resolve_label(&labels, &label))
                        }
                        UnresolvedOp::JumpIfFalse(label) => {
                            Op::JumpIfFalse(Self::resolve_label(&labels, &label))
                        }
                        UnresolvedOp::JumpIfTrue(label) => {
                            Op::JumpIfTrue(Self::resolve_label(&labels, &label))
                        }
                    }
                }
            }
        }).collect();

        CompilerBuild {
            operations,
            consts: self.consts,
            functions,
            strings: self.strings,
            exports: self.exports,
            imports: self.imports
        }
    }
}

pub fn compile_program(ir: &TranslatorOutput, compile_info: CompileInfo) -> CompilerBuild {
    let mut compiler = Compiler::new(compile_info);

    compile_body(ir.program.body.clone(), &mut compiler);
    compiler.emit(Op::Halt);

    compiler.build()
}

pub fn compile_body(statements: Rc<[IRStatement]>, compiler: &mut Compiler) {
    statements.iter().for_each(|statement| compile_statement(statement, compiler));
}

pub fn compile_block(block: &IRBlock, compiler: &mut Compiler) {
    compile_body(block.statements.clone(), compiler);
}

pub fn compile_statement(statement: &IRStatement, compiler: &mut Compiler) {
    match &statement.kind {
        StatementKind::Expression { expression } => compile_expression(expression, ValueUsage::Discard, compiler),
        StatementKind::VariableDefinition { descriptor, expression, .. } => {
            compile_expression(expression, ValueUsage::Needed, compiler);
            for var_ref in descriptor {
                store_variable_ref(var_ref, compiler);
            }
        }
        StatementKind::Function { variable, arguments, block, .. } => {
            let function_info = compiler.function_infos.get(variable).expect("Function infos not found").clone();

            compile_function(&function_info, arguments, block, compiler);
            store_variable_ref(variable, compiler);
        }
        StatementKind::For { variable, iterable, statements } => {
            let loop_start = compiler.generate_label();
            let loop_end = compiler.generate_label();

            match iterable {
                Iterable::Range(start, end) => {
                    let (start, end) = (start.clone().expect("Both sides of a range expression should be defined"), end.clone().expect("Both sides of a range expression should be defined"));

                    compile_expression(&start, ValueUsage::Needed, compiler);
                    compiler.emit(Op::Store(variable.slot));

                    compiler.loops.push(Loop { next_iteration_label: loop_start.clone(), end_label: loop_end.clone() });

                    compiler.label(&loop_start);

                    compiler.emit(Op::Load(variable.slot));
                    compile_expression(&end, ValueUsage::Needed, compiler);
                    compiler.emit(Op::GTE);

                    compiler.jump_if_true(&loop_end);

                    compile_body(statements.clone(), compiler);

                    compiler.emit(Op::Increment(variable.slot));

                    compiler.jump(&loop_start);

                    compiler.label(&loop_end);

                    compiler.loops.pop();
                }
                Iterable::Array(array) => {
                    const ARRAY_SLOT: usize = 0;
                    const LOOP_VARIABLE_SLOT: usize = 1;

                    compile_expression(array, ValueUsage::Needed, compiler);
                    compiler.emit(Op::Store(ARRAY_SLOT));
                    
                    let zero = compiler.constant(Value::Float(0.0));
                    compiler.emit(Op::Const(zero));
                    compiler.emit(Op::Store(LOOP_VARIABLE_SLOT));

                    compiler.loops.push(Loop { next_iteration_label: loop_start.clone(), end_label: loop_end.clone() });

                    compiler.label(&loop_start);
    
                    compiler.emit(Op::Load(LOOP_VARIABLE_SLOT));
                    compiler.emit(Op::Load(ARRAY_SLOT));
                    compiler.emit(Op::Length);
                    compiler.emit(Op::GTE);

                    compiler.jump_if_true(&loop_end);

                    compiler.emit(Op::Load(ARRAY_SLOT));
                    compiler.emit(Op::Load(LOOP_VARIABLE_SLOT));
                    compiler.emit(Op::ReadProperty);
                    compiler.emit(Op::Store(variable.slot));

                    compile_body(statements.clone(), compiler);

                    compiler.emit(Op::Increment(LOOP_VARIABLE_SLOT));

                    compiler.jump(&loop_start);

                    compiler.label(&loop_end);

                    compiler.loops.pop();
                }
            };
        }
        StatementKind::While { condition, statements } => {
            let loop_start = compiler.generate_label();
            let loop_end = compiler.generate_label();

            compiler.label(&loop_start);

            compile_expression(condition, ValueUsage::Needed, compiler);
            compiler.jump_if_false(&loop_end);
            
            compiler.loops.push(Loop { next_iteration_label: loop_start.clone(), end_label: loop_end.clone() });

            compile_body(statements.clone(), compiler);
            compiler.jump(&loop_start);

            compiler.label(&loop_end);

            compiler.loops.pop();
        }
        StatementKind::If { condition, block, alternate } => {
            compile_expression(condition, ValueUsage::Needed, compiler);

            let alternate_label = compiler.generate_label();
            let end_label = compiler.generate_label();

            compiler.jump_if_false(&alternate_label);
            
            compile_block(block, compiler);

            compiler.jump(&end_label);

            compiler.label(&alternate_label);

            if let Some(alternate) = alternate {
                compile_statement(alternate, compiler);
            }

            compiler.label(&end_label);
        }
        StatementKind::Block(block) => {
            compile_block(block, compiler);
        }
        StatementKind::Return { expression } => {
            compile_expression(expression, ValueUsage::Needed, compiler);
            compiler.emit(Op::Return);
        }
        StatementKind::Break => {
            let label = compiler.loops.last().expect("Tried to break on a non-loop statement").end_label.clone();

            compiler.jump(&label);
        }
        StatementKind::Continue => {
            let label = compiler.loops.last().expect("Tried to continue on a non-loop statement").next_iteration_label.clone();

            compiler.jump(&label);
        }
        StatementKind::Import { specifiers, source } => {
            let path: PathBuf = source.value.clone().into();
            let resolved_source  = match compiler.path_resolver.resolve_path(path.clone(), &compiler.source) {
                Ok(path) => path,
                _ => path
            };

            let import_id = compiler.register_import(Import {
                kind: ImportKind::Specific(specifiers.iter().map(|specifier| specifier.name.clone().unwrap()).collect()),
                source: resolved_source.to_str().unwrap().to_string()
            });

            compiler.emit(Op::Import(import_id));

            for specifier in specifiers {
                store_variable_ref(specifier, compiler);
            }
        }
        StatementKind::Export { declaration } => {
            match &declaration.kind {
                StatementKind::Function { variable, arguments, block, .. } => {
                    let function_info = compiler.function_infos.get(variable).expect("Function infos not found").clone();

                    compile_function(&function_info, arguments, block, compiler);
                    compiler.emit(Op::Store(variable.slot));
                    compiler.exports.insert(variable.name.clone().unwrap(), variable.slot);
                }
                StatementKind::Class { variable, .. } => {
                    compile_statement(declaration, compiler);
                    compiler.exports.insert(variable.name.clone().unwrap(), variable.slot);
                }
                _ => {}
            }
        }
        StatementKind::Class { variable, methods, fields, .. } => {
            const SELF_SLOT: usize = 1;

            let class_name = variable.name.as_ref().unwrap();
            let class_info = compiler.class_infos.get(variable).unwrap().clone();

            let mut static_methods = vec![];
            let mut instance_methods = vec![];

            for method in methods.iter() {
                let IRStatementKind::Method { name, arguments, .. } = &method.kind else {
                    unreachable!()
                };

                if name == class_name || arguments.get(0).is_some_and(|argument| argument.name.as_ref().unwrap() == "self") {
                    instance_methods.push(method);
                } else {
                    static_methods.push(method);
                }
            }

            let constructor = instance_methods.iter().find(|method| {
                let Statement { kind: StatementKind::Method { name, .. }, .. } = method else {
                    unreachable!()
                };

                name == class_name
            });

            let Some(Statement { kind: StatementKind::Method { arguments, .. }, .. }) = constructor else {
                unreachable!()
            };

            let constructor_info = class_info.methods.get(class_name).unwrap();

            compiler.begin_function_with(AbstractFunction {
                completed: false,
                instructions: vec![],
                labels: vec![],
                upvalue_descriptors: constructor_info.upvalue_descriptors.clone(),
                argument_count: arguments.len(),
                locals: constructor_info.locals.clone()
            });


            for field in fields.iter().rev() {
                let Statement { kind: ASTStatementKind::Field { visibility: _, name, value }, .. } = field else {
                    unreachable!()
                };

                compiler.string_constant(name);
                compile_expression(value, ValueUsage::Needed, compiler);
            }

            compiler.emit(Op::BuildObject(fields.len()));
            compiler.emit(Op::Store(SELF_SLOT));
            
            for method in instance_methods.iter().rev() {
                let Statement { kind: StatementKind::Method { name, arguments, block }, .. } = method else {
                    unreachable!()
                };

                if name == class_name {
                    continue;
                }

                let method_info = class_info.methods.get(name).unwrap();

                // Load self, load method name, make method closure, bind self to it, write the method to self
                compiler.emit(Op::Load(SELF_SLOT));
                compiler.string_constant(name);

                compile_function(method_info, arguments, block, compiler);
                compiler.emit(Op::Load(SELF_SLOT));
                compiler.emit(Op::BindSelf);
                compiler.emit(Op::WriteProperty);
            }
            
            if let Some(Statement { kind: StatementKind::Method { block, .. }, .. }) = constructor {
                compile_block(block, compiler);
            }

            compiler.emit(Op::Load(SELF_SLOT));
            compiler.emit(Op::Return);

            let function_id = compiler.end_function();

            compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(function_id))));
            compiler.emit(Op::BuildClass);
            store_variable_ref(variable, compiler);

            for method in static_methods.iter().rev() {
                let Statement { kind: StatementKind::Method { name, arguments, block }, .. } = method else {
                    unreachable!()
                };

                let method_info = class_info.methods.get(name).unwrap();

                compiler.emit(Op::Load(variable.slot));
                compiler.string_constant(name);
                compile_function(method_info, arguments, block, compiler);
                compiler.emit(Op::WriteProperty);
            }
        }
        _ => todo!()
    }
}

pub fn compile_expression(expression: &Expression, usage: ValueUsage, compiler: &mut Compiler) {
    match &expression.kind {
        ExpressionKind::Literal { r#type, value } => {
            match r#type {
                LiteralExpressionKind::Literal => {
                    if let Literal::String(string) = value {
                        let string_id = compiler.pre_allocate_string(string.clone());

                        compiler.emit(Op::BuildString(string_id));
                    } else {
                        let value = match value {
                            Literal::Number(number) => Value::Float(*number),
                            Literal::Bool(bool) => Value::Bool(*bool),
                            _ => todo!()
                        };

                        let constant_id = compiler.constant(value);
                        compiler.emit(Op::Const(constant_id));
                    }
                }
                LiteralExpressionKind::Variable => {
                    let Literal::String(string) = value else {
                        unreachable!()
                    };

                    let string_id = compiler.pre_allocate_string(string.to_string());

                    compiler.emit(Op::LoadNative(string_id));
                }
                LiteralExpressionKind::Object(map) => {
                    for (key, value) in map.iter().rev() {
                        compiler.string_constant(key);
                        compile_expression(value, ValueUsage::Needed, compiler);
                    }
                    compiler.emit(Op::BuildObject(map.len()));
                }
                LiteralExpressionKind::Array(values) => {
                    for value in values.iter().rev() {
                        compile_expression(value, ValueUsage::Needed, compiler);
                    }
                    compiler.emit(Op::BuildArray(values.len()));
                }
                LiteralExpressionKind::Function(wrapper) => {
                    let box StatementKindWrapper::IR(StatementKind::Function { variable, arguments, block, .. }) = wrapper else {
                        unreachable!()
                    };

                    let function_info = compiler.function_infos.get(variable).expect("Function infos not found").clone();

                    compile_function(&function_info, arguments, block, compiler);
                }
            }
        }
        ExpressionKind::Binary { left, operator, right } => {
            compile_expression(left, ValueUsage::Needed, compiler);
            compile_expression(right, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::Plus => compiler.emit(Op::Add),
                TokenKind::Minus => compiler.emit(Op::Sub),
                TokenKind::Asterisk => compiler.emit(Op::Mul),
                TokenKind::Slash => compiler.emit(Op::Div),
                TokenKind::Percent => compiler.emit(Op::Mod),
                _ => unreachable!()
            }
        }
        ExpressionKind::Unary { left, operator } => {
            compile_expression(left, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::Minus => {
                    compiler.emit(Op::Neg);
                }
                TokenKind::Bang => {
                    compiler.emit(Op::Not);
                }
                _ => unreachable!()
            }
        }
        ExpressionKind::Relational { left, operator, right } => {
            compile_expression(left, ValueUsage::Needed, compiler);
            compile_expression(right, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::GT => compiler.emit(Op::GT),
                TokenKind::GTE => compiler.emit(Op::GTE),
                TokenKind::LT => compiler.emit(Op::LT),
                TokenKind::LTE => compiler.emit(Op::LTE),
                _ => unreachable!()
            }
        }
        ExpressionKind::Equality { left, operator, right } => {
            compile_expression(left, ValueUsage::Needed, compiler);
            compile_expression(right, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::EQ => compiler.emit(Op::EQ),
                TokenKind::NE => compiler.emit(Op::NEQ),
                _ => unreachable!()
            }
        }
        ExpressionKind::Logical { left, operator, right } => {
            let end_label = compiler.generate_label();

            compile_expression(left, ValueUsage::Needed, compiler);
            compiler.emit(Op::Dup);

            match operator {
                TokenKind::And => {
                    compiler.jump_if_false(&end_label);

                },
                TokenKind::Or => {
                    compiler.jump_if_true(&end_label);
                },
                _ => unreachable!()
            }

            compiler.emit(Op::Pop);
            compile_expression(right, ValueUsage::Needed, compiler);     
            compiler.label(&end_label);
        }
        ExpressionKind::Variable(var_ref) => {
            load_variable_ref(var_ref, compiler);
        }
        ExpressionKind::Call { callee, arguments } => {
            match &callee.kind {
                _ => {
                    for argument in arguments.iter().rev() {
                        compile_expression(argument, ValueUsage::Needed, compiler);
                    }
                    compile_expression(callee, ValueUsage::Needed, compiler);
                    compiler.emit(Op::Call(arguments.len()));
                }
            }
        }
        ExpressionKind::Assignment { left, operator, right } => {
            fn compile_value(left: &Expression, operator: &TokenKind, right: &Expression, compiler: &mut Compiler) {
                match operator {
                    TokenKind::Assign => {
                        compile_expression(right, ValueUsage::Needed, compiler);
                    }
                    TokenKind::PlusEqual => {
                        compile_expression(left, ValueUsage::Needed, compiler);
                        compile_expression(right, ValueUsage::Needed, compiler);
                        compiler.emit(Op::Add);
                    }
                    TokenKind::MinusEqual => {
                        compile_expression(left, ValueUsage::Needed, compiler);
                        compile_expression(right, ValueUsage::Needed, compiler);
                        compiler.emit(Op::Sub);
                    }
                    TokenKind::AsteriskEqual => {
                        compile_expression(left, ValueUsage::Needed, compiler);
                        compile_expression(right, ValueUsage::Needed, compiler);
                        compiler.emit(Op::Mul);
                    }
                    TokenKind::SlashEqual => {
                        compile_expression(left, ValueUsage::Needed, compiler);
                        compile_expression(right, ValueUsage::Needed, compiler);
                        compiler.emit(Op::Div);
                    }
                    TokenKind::PercentEqual => {
                        compile_expression(left, ValueUsage::Needed, compiler);
                        compile_expression(right, ValueUsage::Needed, compiler);
                        compiler.emit(Op::Mod);
                    }
                    _ => unreachable!()
                }
            }

            match &left.kind {
                ExpressionKind::Variable(var_ref) => {
                    if matches!(left.kind, ExpressionKind::Variable(_)) && matches!(right.kind, ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: Literal::Number(1.0) }) {
                        let ExpressionKind::Variable(VariableRef { slot, .. }) = left.kind else {
                            unreachable!()
                        };

                        compiler.emit(Op::Increment(slot));
                        return
                    }

                    compile_value(left, operator, right, compiler);
                    match var_ref.kind {
                        VariableKind::Local => {
                            compiler.emit(Op::Store(var_ref.slot));
                        }
                        VariableKind::Upvalue => {
                            compiler.emit(Op::StoreUpvalue(var_ref.slot));
                        }
                    }
                    return
                }
                ExpressionKind::Member { object, property } => {
                    compile_expression(object, ValueUsage::Needed, compiler);
                    compile_expression(property, ValueUsage::Needed, compiler);
                    compile_value(left, operator, right, compiler);
                    compiler.emit(Op::WriteProperty);
                    return
                }
                _ => todo!()
            }
        }
        ExpressionKind::Member { object, property } => {
            compile_expression(object, ValueUsage::Needed, compiler);
            compile_expression(property, ValueUsage::Needed, compiler);
            compiler.emit(Op::ReadProperty);
        }
    }

    if matches!(usage, ValueUsage::Discard) {
        compiler.emit(Op::Pop);
    }
}

fn compile_function(function_info: &FunctionInfo, arguments: &Vec<VariableRef>, block: &IRBlock, compiler: &mut Compiler) {
    let abstract_function = AbstractFunction {
        completed: false,
        instructions: vec![],
        upvalue_descriptors: function_info.upvalue_descriptors.to_vec(),
        labels: vec![],
        argument_count: arguments.len(),
        locals: function_info.locals.to_vec()
    };

    compiler.begin_function_with(abstract_function);

    compile_block(block, compiler);

    let void_const = compiler.constant(Value::Void);
    compiler.emit(Op::Const(void_const));

    compiler.emit(Op::Return);
    let function_id = compiler.end_function();

    compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(function_id))));
}

fn store_variable_ref(var_ref: &VariableRef, compiler: &mut Compiler) {
    match var_ref.kind {
        VariableKind::Local => {
            compiler.emit(Op::Store(var_ref.slot));
        }
        VariableKind::Upvalue => {
            compiler.emit(Op::StoreCaptured(var_ref.slot));
        }
    }
}

fn load_variable_ref(var_ref: &VariableRef, compiler: &mut Compiler) {
    match var_ref.kind {
        VariableKind::Local => {
            compiler.emit(Op::Load(var_ref.slot));
        }
        VariableKind::Upvalue => {
            compiler.emit(Op::LoadUpvalue(var_ref.slot));
        }
    }
}