use std::{collections::HashMap, fs, rc::Rc};

use reqwest::header::LAST_MODIFIED;

use crate::{compiler, interpreter::prototypes::Operation, lexer::TokenKind, new_translator::TranslatorOutput, parser::{Block, Expression, ExpressionKind, IRBlock, IRProgram, IRStatement, Iterable, LiteralExpressionKind, StatementKind, ValueHolder, VariableRef}, vm::{self, Function, FunctionId, FunctionRef, Op, UpvalueDescriptor, UpvalueSource, Value}};

enum UnresolvedOp {
    Jump(String),
    JumpIfFalse(String)
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
    pub instructions: Vec<Instruction>,
    completed: bool,
    upvalue_descriptors: Vec<UpvalueDescriptor>
}

pub struct CompilerBuild {
    pub operations: Vec<Op>,
    pub consts: Vec<Value>,
    pub functions: Vec<Function>
}

pub struct Compiler {
    labels: HashMap<String, usize>,
    instructions: Vec<Instruction>,
    consts: Vec<Value>,
    functions: Vec<AbstractFunction>,
    label_id: usize,
    function_infos: FunctionInfos
}

pub type FunctionInfos = HashMap<VariableRef, Vec<UpvalueDescriptor>>;

impl Compiler {
    pub fn new(function_infos: FunctionInfos) -> Self {
        Self {
            labels: HashMap::new(),
            instructions: vec![],
            consts: vec![],
            functions: vec![],
            label_id: 0,
            function_infos
        }
    }

    pub fn label(&mut self, name: &str) {
        self.labels.insert(name.to_string(), self.instructions.len());
    }

    pub fn new_label(&mut self) -> String {
        let name = self.label_id.to_string();

        self.label(&name);
        self.label_id += 1;

        name
    }

    fn emit_instruction(&mut self, instruction: Instruction) {
        for function in self.functions.iter_mut().rev() {
            if function.completed {
                continue;
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

    pub fn begin_function(&mut self, upvalue_descriptors: Vec<UpvalueDescriptor>) {
        self.functions.push(AbstractFunction { instructions: vec![], completed: false, upvalue_descriptors });
    }

    pub fn end_function(&mut self) -> usize {
        for (index, function) in self.functions.iter_mut().rev().enumerate() {
            if function.completed {
                continue;
            }

            function.completed = true;

            return self.functions.len() - index - 1
        }
        
        unreachable!()
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
        let labels = self.labels;

        let mut functions: Vec<Function> = vec![];

        for (_, function) in self.functions.iter_mut().enumerate() {
            functions.push(Function {
                // TODO: use real values
                module_id: 0,
                argument_count: 0,
                local_count: 5,
                code_offset: self.instructions.len(),
                upvalue_descriptors: function.upvalue_descriptors.clone()
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
                    }
                }
            }
        }).collect();

        fs::write(format!("build.dbg"), operations.iter().map(|op| format!("{op:?}")).collect::<Vec<String>>().join("\n")).unwrap();

        CompilerBuild {
            operations,
            consts: self.consts,
            functions
        }
    }
}

pub fn compile_program(ir: TranslatorOutput) -> CompilerBuild {
    let mut compiler = Compiler::new(ir.function_infos);

    compile_body(ir.program.body, &mut compiler);
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
                if var_ref.upvalue {
                    compiler.emit(Op::StoreCaptured(var_ref.slot));
                } else {
                    compiler.emit(Op::Store(var_ref.slot));
                }
            }
        }
        StatementKind::Function { variable, arguments, block, .. } => {
            compiler.begin_function(compiler.function_infos.get(variable).expect("Function infos not found").to_vec());

            for argument in arguments.iter().rev() {
                compiler.emit(Op::Load(argument.slot));
            }

            compile_block(block, compiler);

            let void_const = compiler.constant(Value::Void);
            compiler.emit(Op::Const(void_const));

            compiler.emit(Op::Return);
            let function_id = compiler.end_function();

            compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(function_id))));
            compiler.emit(Op::Store(variable.slot));
        }
        StatementKind::For { variable, iterable, statements } => {
            let (start, end) = match iterable {
                Iterable::Range(start, end) => {
                    (start.clone().unwrap(), end.clone().unwrap())
                }
                _ => todo!()
            };
            
            compile_expression(&start, ValueUsage::Needed, compiler);
            compiler.emit(Op::Store(variable.slot));

            let loop_start = compiler.new_label();

            compile_body(statements.clone(), compiler);
            compiler.emit(Op::Increment(variable.slot));

            compiler.emit(Op::Load(variable.slot));
            compile_expression(&end, ValueUsage::Needed, compiler);
            compiler.emit(Op::GTE);

            compiler.jump_if_false(&loop_start);
        }
        _ => todo!()
    }
}

pub fn compile_expression(expression: &Expression, usage: ValueUsage, compiler: &mut Compiler) {
    match &expression.kind {
        ExpressionKind::Literal { r#type, value } => {
            match r#type {
                LiteralExpressionKind::Literal => {
                    let constant_id = compiler.constant(match value {
                        ValueHolder::String(string) => Value::String(Rc::new(string.to_string())),
                        ValueHolder::Number(number) => Value::Float(*number),
                        ValueHolder::Bool(bool) => Value::Bool(*bool),
                        _ => todo!()
                    });
                    compiler.emit(Op::Const(constant_id));
                }
                LiteralExpressionKind::Variable => {
                    println!("{}", value)
                }
                LiteralExpressionKind::Object(map) => {
                    for (key, value) in map.iter().rev() {
                        compiler.constant(Value::String(Rc::new(key.to_string())));
                        compile_expression(value, ValueUsage::Needed, compiler);
                    }
                    compiler.emit(Op::MakeObject(map.len()));
                }
                _ => todo!()
            }
        }
        ExpressionKind::Binary { left, operator, right } => {
            compile_expression(left, ValueUsage::Needed, compiler);
            compile_expression(right, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::Plus => {
                    compiler.emit(Op::Add);
                },
                _ => todo!()
            }
        }
        ExpressionKind::Variable(var_ref) => {
            if var_ref.upvalue {
                compiler.emit(Op::LoadUpvalue(var_ref.slot));
            } else {
                compiler.emit(Op::Load(var_ref.slot));
            }
        }
        ExpressionKind::Call { callee, arguments } => {
            match &callee.kind {
                ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } => {
                    if name == "print" {
                        compile_expression(&arguments[0], ValueUsage::Needed, compiler);
                        compiler.emit(Op::CallNative(vm::print));
                    }
                }
                _ => {
                    compile_expression(callee, ValueUsage::Needed, compiler);
                    for argument in arguments {
                        compile_expression(argument, ValueUsage::Needed, compiler);
                    }
                    compiler.emit(Op::Call(arguments.len()));
                }
            }
        }
        ExpressionKind::Assignment { left, right, .. } => {
            compile_expression(right, ValueUsage::Needed, compiler);
            
            match &left.kind {
                ExpressionKind::Variable(var_ref) => {
                    if var_ref.upvalue {
                        compiler.emit(Op::StoreUpvalue(var_ref.slot));
                    } else {
                        compiler.emit(Op::Store(var_ref.slot));
                    }
                    return;
                }
                _ => todo!()
            }
        }
        _ => todo!()
    }

    if matches!(usage, ValueUsage::Discard) {
        compiler.emit(Op::Pop);
    }
}