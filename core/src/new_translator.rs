// This version of the translator was fixed to work with the NLF compiler

use std::{collections::HashMap, rc::Rc, vec};

use serde::Serialize;

use crate::{compiler::{ClassInfo, ClassInfos, FunctionInfo, FunctionInfos}, errors::{LanguageError, LanguageErrorKind, LanguageResult}, explorer::{self, Visitor}, interpreter::prototypes::Operation, lexer::TokenKind, parser::{ASTProgram, ASTStatement, ASTStatementKind, ASTSyntaxTree, Block, Expression, ExpressionKind, IRProgram, IRStatement, IRStatementKind, Iterable, LiteralExpressionKind, Program, Statement, StatementKind, StatementKindWrapper, ValueHolder, VariableDescriptor, VariableKind, VariableRef}, vm::{UpvalueDescriptor, UpvalueSource}};

#[derive(Debug)]
pub enum TranslatorError {
    UnknownVariable(String),
    TODO(String)
}

impl LanguageErrorKind for TranslatorError {}

#[derive(Serialize, Debug, Clone)]
enum ScopeKind {
    Program,
    Loop,
    Conditional,
    Function,
    Block
}

#[derive(Serialize, Debug, Clone, Copy)]
struct Symbol {
    pub slot: usize,
    pub upvalue: bool,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
struct SymbolTable {
    map: HashMap<String, Symbol>
}

impl SymbolTable {
    fn new() -> Self {
        SymbolTable { map: HashMap::new() }
    }

    fn get(&self, name: &str) -> Option<Symbol> {
        self.map.get(name).copied()
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.map.get_mut(name)
    }

    fn set(&mut self, name: &str, position: (usize, usize)) -> usize {
        let id = self.map.len();
        self.map.insert(name.to_string(), Symbol { slot: id, start: position.0, end: position.1, upvalue: false });
        id
    }
}

#[derive(Serialize, Debug, Clone)]
struct Scope {
    kind: ScopeKind,
    slot_index: usize,
    symbol_table: SymbolTable
}


#[derive(Serialize, Debug, Clone)]
struct Frame {
    symbol_table: SymbolTable,
    upvalue_descriptors: Vec<UpvalueDescriptor>
}

impl Frame {
    pub fn get_or_register_upvalue(&mut self, upvalue_descriptor: UpvalueDescriptor) -> usize {
        for (index, descriptor) in self.upvalue_descriptors.iter().enumerate() {
            if descriptor.index == upvalue_descriptor.index && descriptor.source == upvalue_descriptor.source {
                return index
            }
        }

        let index = self.upvalue_descriptors.len();

        self.upvalue_descriptors.push(upvalue_descriptor);

        index
    }
}

#[derive(Serialize, Debug, Clone)]
struct Context {
    stack: Vec<Scope>,
    frames: Vec<Frame>,
    program: ASTProgram,
    function_infos: FunctionInfos,
    class_infos: ClassInfos
}

impl Context {
    fn get(&mut self, name: &str) -> Option<VariableRef> {
        let mut scopes = self.stack.iter().rev();

        let mut found_variable = true;
        while let Some(scope) = scopes.next() {
            if let Some(_) = scope.symbol_table.get(name) {
                found_variable = true;
                break;
            }
        }

        if !found_variable {
            return None;
        }

        let frames = self.frames.clone();
        let mut frames_iter = frames.iter().rev();

        let mut depth = 0;
        while let Some(frame) = frames_iter.next() {
            if let Some(symbol) = frame.symbol_table.get(name) {
                // This is the definition frame, we need to go back to the access frame and register upvalue descriptors
                let mut upvalue_index = symbol.slot;
                for i in (frames.len() - depth)..self.frames.len() {
                    let frame = self.frames.get_mut(i).expect("Frame not found");

                    let new_descriptor = if i == frames.len() - depth {
                        UpvalueDescriptor {
                            index: symbol.slot,
                            source: UpvalueSource::Local
                        }
                    } else {
                        UpvalueDescriptor {
                            index: upvalue_index,
                            source: UpvalueSource::Upvalue
                        }
                    };
                    
                    upvalue_index = frame.get_or_register_upvalue(new_descriptor);
                }

                let kind = if depth > 0 && symbol.upvalue {
                    VariableKind::Upvalue
                } else {
                    VariableKind::Local
                };

                return Some(VariableRef { kind, name: None, slot: upvalue_index, depth, start: symbol.start, end: symbol.end });
            }

            depth += 1;
        }

        None
    }

    fn set_with_position(&mut self, name: &str, position: (usize, usize)) -> VariableRef {
        let is_upvalue = self.is_upvalue(name, position);

        let scope = self.stack.last_mut().unwrap();

        scope.symbol_table.set(name, position);

        let frame = self.frames.last_mut().unwrap();

        let id = frame.symbol_table.set(name, (0, 0));

        if is_upvalue {
            let symbol = frame.symbol_table.get_mut(name);

            if let Some(symbol) = symbol {
                symbol.upvalue = true;
            }
        }

        let kind = if is_upvalue {
            VariableKind::Upvalue
        } else {
            VariableKind::Local
        };

        // TOOD: refactor
        VariableRef { kind, name: Some(name.to_owned()), slot: id, depth: 0, start: position.0, end: position.1 }
    }
    
    fn set(&mut self, name: &str) -> VariableRef {
        self.set_with_position(name, (0, 0))
    }
    
    fn enter_scope(&mut self, scope: ScopeKind) {
        self.stack.push(Scope { kind: scope, slot_index: 0, symbol_table: SymbolTable::new() });
    }

    fn exit_scope(&mut self) {
        self.stack.pop();
    }

    fn enter_frame(&mut self) {
        self.frames.push(Frame { symbol_table: SymbolTable::new(), upvalue_descriptors: vec![] });
    }

    fn exit_frame(&mut self) -> FunctionInfo {
        let frame = self.frames.last().unwrap();

        let function_info = FunctionInfo {
            upvalue_descriptors: frame.upvalue_descriptors.clone(),
            local_count: frame.symbol_table.map.len()
        };

        self.frames.pop();

        function_info
    }

    fn is_upvalue(&self, name: &str, position: (usize, usize)) -> bool {
        #[derive(Debug)]
        struct Frame;

        struct FrameExplorer {
            frames: Vec<Frame>,
            name: String,
            definition: Option<(ASTStatement, usize)>,
            position: (usize, usize),
            upvalue: bool
        }

        impl Visitor<ASTSyntaxTree> for FrameExplorer {
            fn visit_expression(&mut self, expression: &Expression) {
                match &expression.kind {
                    ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(_), .. } => {
                        self.frames.push(Frame);
                    }
                    ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(variable_name) } => {
                        if self.name != *variable_name {
                            return
                        }

                        if self.definition.as_ref().unwrap().1 != self.frames.len() {
                            self.upvalue = true
                        }
                    }
                    _ => {}
                }
            }

            fn visit_statement(&mut self, statement: &ASTStatement) {
                if self.position.0 >= statement.start && self.position.1 <= statement.end {
                    self.definition = Some((statement.clone(), self.frames.len()));
                }

                match &statement.kind {
                    ASTStatementKind::Function { .. } => {
                        self.frames.push(Frame);
                    }
                    _ => {}
                }
            }

            fn leave_expression(&mut self, expression: &Expression) {
                match &expression.kind {
                    ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(_), .. } => {
                        self.frames.pop();
                    }
                    _ => {}
                }
            }

            fn leave_statement(&mut self, statement: &ASTStatement) {
                match &statement.kind {
                    ASTStatementKind::Function { .. } => {
                        self.frames.pop();
                    }
                    _ => {}
                }
            }
        }

        let mut visitor = FrameExplorer {
            frames: vec![Frame],
            name: name.to_string(),
            definition: None,
            position,
            upvalue: false
        };

        explorer::visit_program(&self.program, &mut visitor);

        visitor.upvalue
    }
}

pub struct TranslatorOutput {
    pub program: IRProgram,
    pub function_infos: FunctionInfos,
    pub class_infos: ClassInfos,
    pub local_count: usize
}

pub fn translate(program: ASTProgram) -> LanguageResult<TranslatorOutput> {
    let mut context = Context {
        stack: vec![Scope { kind: ScopeKind::Program, slot_index: 0, symbol_table: SymbolTable::new() }],
        frames: vec![Frame { symbol_table: SymbolTable::new(), upvalue_descriptors: vec![] }],
        program: program.clone(),
        function_infos: FunctionInfos::new(),
        class_infos: ClassInfos::new()
    };

    translate_body(program.body, &mut context).map(|statements| {
        TranslatorOutput { program: Program { body: statements }, function_infos: context.function_infos, local_count: context.frames.last().unwrap().symbol_table.map.len(), class_infos: context.class_infos }
    })
}

fn translate_body(statements: Rc<[ASTStatement]>, context: &mut Context) -> LanguageResult<Rc<[IRStatement]>> {
    let mut inner_statements: Vec<IRStatement> = vec![];
    
    for statement in statements.iter() {
        inner_statements.push(translate_statement(statement.clone(), context)?);
    }

    Ok(inner_statements.into())
}

fn translate_statement(statement: ASTStatement, context: &mut Context) -> LanguageResult<IRStatement> {
    let new_statement_kind = match statement.kind {
        ASTStatementKind::Expression { expression } => {
            StatementKind::Expression { expression: translate_expression(expression, context)? }
        },
        ASTStatementKind::If { condition, block, alternate } => {
            let condition = translate_expression(condition, context)?;

            context.enter_scope(ScopeKind::Conditional);
            let block = translate_block(block, context)?;
            context.exit_scope();

            let alternate = if let Some(alt) = alternate {
                Some(Box::new(translate_statement(*alt, context)?))
            } else {
                None
            };

            StatementKind::If { condition, block, alternate }
        },
        ASTStatementKind::Function { variable, arguments, block, return_type_ref, return_ty } => {
            let var_ref = context.set(&variable.value);
            context.enter_frame();
            context.enter_scope(ScopeKind::Function);
            context.set(&variable.value);
            context.set("self");
            let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
            let block = translate_block(block, context)?;
            context.exit_scope();
            let function_info = context.exit_frame();

            context.function_infos.insert(var_ref.clone(), function_info);

            StatementKind::Function { variable: var_ref, arguments: inner_arguements, block, return_type_ref, return_ty }
        },
        ASTStatementKind::Block(block) => {
            context.enter_scope(ScopeKind::Block);
            let block = translate_block(block, context)?;
            context.exit_scope();

            StatementKind::Block(block)
        }
        ASTStatementKind::For { variable, iterable, statements } => {
            let iterable = match iterable {
                Iterable::Range(left, right) => {
                    let left = left.map(|left| translate_expression(left, context)).transpose()?;
                    let right = right.map(|right| translate_expression(right, context)).transpose()?;

                    Iterable::Range(left, right)
                }
                Iterable::Array(expression) => {
                    Iterable::Array(translate_expression(expression, context)?)
                }
            };

            context.enter_scope(ScopeKind::Loop);
            let var_ref = context.set(&variable.value);
            let statements = translate_body(statements, context)?;
            context.exit_scope();

            StatementKind::For { variable: var_ref, iterable, statements }
        }
        ASTStatementKind::While { condition, statements } => {
            context.enter_scope(ScopeKind::Loop);
            let condition = translate_expression(condition, context)?;
            let statements = translate_body(statements, context)?;
            context.exit_scope();

            StatementKind::While { condition, statements }
        }
        ASTStatementKind::Return { expression } => {
            StatementKind::Return { expression: translate_expression(expression, context)? }
        }
        ASTStatementKind::Import { specifiers, source } => {
            let specifiers= specifiers.iter().map(|specifier| {
                context.set(&specifier.value)
            }).collect();

            StatementKind::Import { specifiers, source }
        }
        ASTStatementKind::Export { declaration } => {
            StatementKind::Export { declaration: Box::new(translate_statement(*declaration, context)?) }
        }
        ASTStatementKind::Class { variable: class_name, methods, fields, body_start, body_end } => {
            let var_ref = context.set(&class_name.value);

            let mut class_info = ClassInfo {
                methods: HashMap::new()
            };

            let mut static_methods = vec![];
            let mut instance_methods = vec![];

            for method in methods.iter() {
                let ASTStatementKind::Function { variable, arguments, .. } = &method.kind else {
                    unreachable!()
                };

                if variable.value == class_name.value || arguments.get(0).is_some_and(|argument| argument.variable.value == "self") {
                    instance_methods.push(method.clone());
                } else {
                    static_methods.push(method.clone());
                }
            }

            let mut translated_methods: Vec<IRStatement> = vec![];

            // We enter the constructor frame because all method closures are instanciated in the constructor
            context.enter_frame();

            let constructor = methods.iter().find(|method| {
                let ASTStatementKind::Function { variable, .. } = &method.kind else {
                    unreachable!()
                };

                variable.value == class_name.value
            });

            // We enter the constructor scope to not make constrcutor variables accessible in methods
            context.enter_scope(ScopeKind::Function);
            // This should be the class definition
            context.set("0");
            context.set("self");

            let constructor_method = if let Some(Statement { kind: ASTStatementKind::Function { variable, arguments, block, .. }, start, end }) = constructor {
                let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                let block = translate_block(block.clone(), context)?;
                
                IRStatementKind::Method { name: variable.value.clone(), arguments: inner_arguements, block }
                    .into_statement(*start, *end)
            } else {
                IRStatementKind::Method { name: class_name.value.clone(), arguments: vec![], block: Block { statements: Rc::new([]), start: 0, end: 0 } }
                    .into_statement(0, 0)
            };
            translated_methods.push(constructor_method);

            context.exit_scope();

            for method in instance_methods {
                let ASTStatementKind::Function { variable, arguments, block, .. } = method.kind else {
                    unreachable!()
                };

                if variable.value == class_name.value {
                    continue;
                }

                // Each method has it's own scope and frame
                context.enter_frame();
                context.enter_scope(ScopeKind::Function);

                context.set(&variable.value);

                let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                let block = translate_block(block, context)?;
                context.exit_scope();
                let method_info = context.exit_frame();

                class_info.methods.insert(variable.value.to_string(), method_info);

                translated_methods.push(IRStatement {
                    kind: IRStatementKind::Method { name: variable.value, arguments: inner_arguements, block },
                    start: method.start,
                    end: method.end
                });
            }

            let constructor_info = context.exit_frame();

            class_info.methods.insert(class_name.value, constructor_info);

            for method in static_methods {
                let ASTStatementKind::Function { variable, arguments, block, .. } = method.kind else {
                    unreachable!()
                };

                context.enter_frame();
                context.enter_scope(ScopeKind::Function);

                context.set(&variable.value);
                context.set("self");

                let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                let block = translate_block(block, context)?;
                context.exit_scope();
                let method_info = context.exit_frame();

                class_info.methods.insert(variable.value.to_string(), method_info);

                translated_methods.push(IRStatement {
                    kind: IRStatementKind::Method { name: variable.value, arguments: inner_arguements, block },
                    start: method.start,
                    end: method.end
                });
            }

            context.class_infos.insert(var_ref.clone(), class_info);

            StatementKind::Class { variable: var_ref, methods: translated_methods, fields, body_start, body_end }
        }
        ASTStatementKind::VariableDefinition { descriptor, expression, type_ref, ty } => {
            let variables = match descriptor {
                VariableDescriptor::Identifier(identifier) => vec![identifier],
                VariableDescriptor::Object(_) => todo!()
            }
                .iter()
                .map(|identifier| context.set_with_position(&identifier.value, (identifier.start, identifier.end)))
                .collect();

            StatementKind::VariableDefinition { descriptor: variables, expression: translate_expression(expression, context)?, type_ref, ty }
        }
        ASTStatementKind::Break => StatementKind::Break,
        ASTStatementKind::Continue => StatementKind::Continue,
        ASTStatementKind::Method { name, arguments, block } => StatementKind::Method { name, arguments, block },
        ASTStatementKind::Field { visibility, name, value } => StatementKind::Field { visibility, name, value }
    };
    Ok(IRStatement {
        kind: new_statement_kind,
        start: statement.start,
        end: statement.end
    })
}

fn translate_expression(mut expression: Expression, context: &mut Context) -> LanguageResult<Expression> {
    expression.kind = match &expression.kind {
        ExpressionKind::Assignment { left, operator, right } => {
            let mut variable = *left.clone();

            // Find the innermost literal variable name
            loop {
                if matches!(&variable.kind, ExpressionKind::Literal { .. }) {
                    break;
                }
                if let ExpressionKind::Member { object, .. } = &variable.kind {
                    variable = object.as_ref().clone();
                } else {
                    break;
                }
            }

            let ExpressionKind::Literal { value: ValueHolder::String(_name), ..  } = &variable.kind else {
                return Err(LanguageError::from(TranslatorError::TODO("Cannot access property on type other than a variable".to_string())));
            };

            ExpressionKind::Assignment { left: Box::new(translate_expression(*left.clone(), context)?), operator: operator.clone(), right: Box::new(translate_expression(*right.clone(), context)?) }
        }
        ExpressionKind::Member { object, property } => {
            let object = translate_expression(*object.clone(), context)?;
            let property = translate_expression(*property.clone(), context)?;

            ExpressionKind::Member {
                object: Box::new(object),
                property: Box::new(property),
            }
        }
        ExpressionKind::Literal { r#type, value } => {
            match r#type {
                LiteralExpressionKind::Variable => {
                    let ValueHolder::String(value) = value else {
                        panic!()
                    };

                    let variable_ref = context.get(value).ok_or_else(|| LanguageError::from(TranslatorError::UnknownVariable(value.clone())))?;

                    return Ok(ExpressionKind::Variable(variable_ref).into_expression(expression.start, expression.end));
                }
                LiteralExpressionKind::Function(statement) => {
                    let box StatementKindWrapper::AST(ASTStatementKind::Function { variable, arguments, block, return_type_ref, return_ty }) = statement.clone() else {
                        panic!()
                    };

                    let var_ref = context.set(&variable.value);
                    context.enter_frame();
                    context.enter_scope(ScopeKind::Function);
                    context.set(&variable.value);
                    context.set("self");
                    let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                    let block = translate_block(block, context)?;
                    context.exit_scope();
                    let function_info = context.exit_frame();

                    context.function_infos.insert(var_ref.clone(), function_info);

                    return Ok(ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(Box::new(StatementKindWrapper::IR(IRStatementKind::Function {
                        variable: var_ref,
                        arguments: inner_arguements,
                        block,
                        return_type_ref,
                        return_ty
                    }))), value: value.clone() }.into_expression(expression.start, expression.end))
                }
                LiteralExpressionKind::Array(array) => {
                    return Ok(ExpressionKind::Literal {
                        r#type: LiteralExpressionKind::Array(array.iter().map(|expression| translate_expression(expression.clone(), context).unwrap()).collect()),
                        value: value.clone()
                    }.into_expression(expression.start, expression.end))
                }
                LiteralExpressionKind::Object(array) => {
                    return Ok(ExpressionKind::Literal {
                        r#type: LiteralExpressionKind::Object(array.iter().map(|(key, expression)| (key.clone(), translate_expression(expression.clone(), context).unwrap())).collect()),
                        value: value.clone()
                    }.into_expression(expression.start, expression.end))
                }
                _ => ()
            }

            expression.kind
        }
        ExpressionKind::Binary { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            if let (ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: left_value }, ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: right_value }) = (&left.kind, &right.kind) {
                let operation = match operator {
                    TokenKind::Plus => Operation::Addition,
                    TokenKind::Minus => Operation::Substraction,
                    TokenKind::Asterisk => Operation::Multiplication,
                    TokenKind::Slash => Operation::Division,
                    TokenKind::Percent => Operation::Modulo,
                    _ => unreachable!()
                };
                return Ok(ExpressionKind::Literal {
                    r#type: LiteralExpressionKind::Literal,
                    value: left_value.get_prototype().operate(operation, left_value, right_value)?
                }.into_expression(left.start, right.end))
            }

            ExpressionKind::Binary {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            }
        }
        ExpressionKind::Logical { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            ExpressionKind::Logical {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            }
        }
        ExpressionKind::Equality { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            ExpressionKind::Equality {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            }
        }
        ExpressionKind::Relational { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            ExpressionKind::Relational {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            }
        }
        ExpressionKind::Unary { left, operator } => {
            let left = translate_expression(*left.clone(), context)?;

            ExpressionKind::Unary {
                left: Box::new(left),
                operator: operator.clone()
            }
        }
        ExpressionKind::Call { callee, arguments } => {
            let callee = match translate_expression(*callee.clone(), context) {
                Ok(expr) => expr,
                Err(_) => *callee.clone(),
            };
            
            let mut inner_arguments: Vec<Expression> = vec![];

            for arg in arguments {
                inner_arguments.push(translate_expression(arg.clone(), context)?);
            }

            ExpressionKind::Call {
                callee: Box::new(callee),
                arguments: inner_arguments,
            }
        }
        _ => expression.kind
    };
    Ok(expression)
}

fn translate_block(block: Block<ASTStatement>, context: &mut Context) -> LanguageResult<Block<IRStatement>> {
    let translated_block = Block {
        statements: translate_body(block.statements, context)?,
        start: block.start,
        end: block.end
    };
    Ok(translated_block)
}