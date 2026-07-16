use std::{collections::HashMap, fs, rc::Rc};

use nlf_shared::indexmap::map::raw_entry_v1::RawOccupiedEntryMut;
use serde::Serialize;

use crate::{lexer::TokenKind, new_translator::TranslatorOutput, parser::{Expression, ExpressionKind, IRBlock, IRStatement, Iterable, LiteralExpressionKind, StatementKind, StatementKindWrapper, ValueHolder, VariableRef}, vm::{self, Function, FunctionId, FunctionRef, Heap, Op, UpvalueDescriptor, Value}};

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
    instructions: Vec<Instruction>,
    completed: bool,
    upvalue_descriptors: Vec<UpvalueDescriptor>,
    labels: Vec<String>,
    argument_count: usize, 
    local_count: usize
}

pub struct CompilerBuild {
    pub operations: Vec<Op>,
    pub consts: Vec<Value>,
    pub functions: Vec<Function>,
    pub heap: Heap
}

pub struct Compiler {
    labels: HashMap<String, usize>,
    instructions: Vec<Instruction>,
    consts: Vec<Value>,
    functions: Vec<AbstractFunction>,
    label_id: usize,
    function_infos: FunctionInfos,
    starting_heap_id: usize,
    heap: Heap,
    loops: Vec<Loop>
}

struct Loop {
    next_iteration_label: String,
    end_label: String
}

pub type FunctionInfos = HashMap<VariableRef, FunctionInfo>;

#[derive(Serialize, Debug, Clone)]
pub struct FunctionInfo {
    pub upvalue_descriptors: Vec<UpvalueDescriptor>,
    pub local_count: usize
}

impl Compiler {
    pub fn new(function_infos: FunctionInfos, starting_heap_id: usize) -> Self {
        Self {
            labels: HashMap::new(),
            instructions: vec![],
            consts: vec![],
            functions: vec![],
            label_id: 0,
            function_infos,
            starting_heap_id,
            heap: Heap::new(),
            loops: vec![]
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

    pub fn begin_function_with(&mut self, abstract_function: AbstractFunction) {
        self.functions.push(abstract_function);
    }

    pub fn begin_function(&mut self, upvalue_descriptors: Vec<UpvalueDescriptor>) {
        self.begin_function_with(AbstractFunction { instructions: vec![], completed: false, upvalue_descriptors, labels: vec![], local_count: 0, argument_count: 0 });
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

    /// Pre allocates a string and returns the heap object id
    pub fn pre_allocate_string(&mut self, string: String) -> usize {
        self.heap.allocate_string(string) + self.starting_heap_id
    }

    pub fn string_constant(&mut self, string: &str) -> usize {
        let heap_id = self.pre_allocate_string(string.to_string());

        self.constant(Value::String(heap_id))
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
                // TODO: use real values
                module_id: 0,
                argument_count: 0,
                local_count: function.local_count,
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

        fs::write(format!("core/debug/vm/build.dbg"), operations.iter().map(|op| format!("{op:?}")).collect::<Vec<String>>().join("\n")).unwrap();

        CompilerBuild {
            operations,
            consts: self.consts,
            functions,
            heap: self.heap
        }
    }
}

pub fn compile_program(ir: TranslatorOutput) -> CompilerBuild {
    let mut compiler = Compiler::new(ir.function_infos, 0);

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
            compile_function(variable, arguments, block, compiler);
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

            let loop_start = compiler.generate_label();
            let loop_next_iteration = compiler.generate_label();
            let loop_end = compiler.generate_label();

            compiler.loops.push(Loop { next_iteration_label: loop_next_iteration.clone(), end_label: loop_end.clone() });

            compiler.label(&loop_start);

            compile_body(statements.clone(), compiler);

            compiler.label(&loop_next_iteration);

            compiler.emit(Op::Increment(variable.slot));

            compiler.emit(Op::Load(variable.slot));
            compile_expression(&end, ValueUsage::Needed, compiler);
            compiler.emit(Op::GTE);

            compiler.jump_if_false(&loop_start);

            compiler.label(&loop_end);

            compiler.loops.pop();
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
        _ => todo!()
    }
}

pub fn compile_expression(expression: &Expression, usage: ValueUsage, compiler: &mut Compiler) {
    match &expression.kind {
        ExpressionKind::Literal { r#type, value } => {
            match r#type {
                LiteralExpressionKind::Literal => {
                    let value = match value {
                        ValueHolder::String(string) => {
                            let heap_id = compiler.pre_allocate_string(string.clone());
                            Value::String(heap_id)
                        },
                        ValueHolder::Number(number) => Value::Float(*number),
                        ValueHolder::Bool(bool) => Value::Bool(*bool),
                        _ => todo!()
                    };
                    let constant_id = compiler.constant(value);
                    compiler.emit(Op::Const(constant_id));
                }
                LiteralExpressionKind::Variable => {
                    println!("{}", value)
                }
                LiteralExpressionKind::Object(map) => {
                    for (key, value) in map.iter().rev() {
                        let key_constant = compiler.string_constant(key);
                        compiler.emit(Op::Const(key_constant));
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

                    compile_function(variable, arguments, block, compiler);
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
            compile_expression(left, ValueUsage::Needed, compiler);
            compile_expression(right, ValueUsage::Needed, compiler);

            match operator {
                TokenKind::And => compiler.emit(Op::And),
                TokenKind::Or => compiler.emit(Op::Or),
                _ => unreachable!()
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
                    for argument in arguments.iter().rev() {
                        compile_expression(argument, ValueUsage::Needed, compiler);
                    }
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
                    _ => unreachable!()
                }
            }

            match &left.kind {
                ExpressionKind::Variable(var_ref) => {
                    compile_value(left, operator, right, compiler);
                    if var_ref.upvalue {
                        compiler.emit(Op::StoreUpvalue(var_ref.slot));
                    } else {
                        compiler.emit(Op::Store(var_ref.slot));
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
        _ => todo!("Expression kind not covered")
    }

    if matches!(usage, ValueUsage::Discard) {
        compiler.emit(Op::Pop);
    }
}

fn compile_function(variable: &VariableRef, arguments: &Vec<VariableRef>, block: &IRBlock, compiler: &mut Compiler) {
    let function_infos = compiler.function_infos.get(variable).expect("Function infos not found");

    let abstract_function = AbstractFunction {
        completed: false,
        instructions: vec![],
        upvalue_descriptors: function_infos.upvalue_descriptors.to_vec(),
        labels: vec![],
        local_count: function_infos.local_count,
        argument_count: arguments.len()
    };

    compiler.begin_function_with(abstract_function);

    compile_block(block, compiler);

    let void_const = compiler.constant(Value::Void);
    compiler.emit(Op::Const(void_const));

    compiler.emit(Op::Return);
    let function_id = compiler.end_function();

    compiler.emit(Op::MakeClosure(FunctionRef::Id(FunctionId(function_id))));
}