use std::{collections::HashMap, rc::Rc, vec};

use serde::Serialize;

use crate::{errors::{LanguageError, LanguageErrorTrait, LanguageResult}, interpreter::prototypes::Operation, lexer::TokenKind, parser::{ASTProgram, ASTStatement, ASTStatementKind, Block, Expression, ExpressionKind, IRProgram, IRStatement, IRStatementKind, Iterable, LiteralExpressionKind, Program, StatementKind, StatementKindWrapper, ValueHolder, VariableDescriptor, VariableRef}};

#[derive(Debug)]
pub enum TranslatorError {
    UnknownVariable(String),
    TODO(String)
}

impl LanguageErrorTrait for TranslatorError {}

#[derive(Serialize, Debug, Clone)]
enum ScopeKind {
    Program,
    Call(usize),
    Loop,
    Conditional,
    Function,
    Block
}

#[derive(Serialize, Debug, Clone, Copy)]
struct Symbol {
    pub slot: usize,
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

    fn set(&mut self, name: &str, position: (usize, usize)) -> usize {
        let id = self.map.len();
        self.map.insert(name.to_string(), Symbol { slot: id, start: position.0, end: position.1 });
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
struct Context {
    stack: Vec<Scope>
}

impl Context {
    fn get(&self, name: &str) -> Option<VariableRef> {
        let mut scopes = self.stack.iter().rev();

        let mut depth = 0;
        while let Some(scope) = scopes.next() {
            if let ScopeKind::Call(idx) = scope.kind {
                scopes = self.stack[0..((idx + 1) as usize)].iter().rev();
            }

            if let Some(symbol) = scope.symbol_table.get(name) {
                return Some(VariableRef { name: None, slot: symbol.slot, depth, start: symbol.start, end: symbol.end });
            }

            depth += 1;
        }

        None
    }

    fn set_with_position(&mut self, name: &str, position: (usize, usize)) -> VariableRef {
        let scope = self.stack.last_mut().unwrap();

        let id = scope.symbol_table.set(name, position);

        // TOOD: refactor
        VariableRef { name: Some(name.to_owned()), slot: id, depth: 0, start: position.0, end: position.1 }
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
}

pub fn translate(program: ASTProgram) -> LanguageResult<IRProgram> {
    let mut context = Context {
        stack: vec![Scope { kind: ScopeKind::Program, slot_index: 0, symbol_table: SymbolTable::new() }]
    };

    translate_body(program.body, &mut context).map(|statements| Program { body: statements })
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
            context.enter_scope(ScopeKind::Function);
            context.set(&variable.value);
            let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
            let block = translate_block(block, context)?;
            context.exit_scope();

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

            let mut translated_methods: Vec<IRStatement> = vec![];
            for method in methods {
                let ASTStatementKind::Function { variable, arguments, block, .. } = method.kind else {
                    unreachable!()
                };

                context.enter_scope(ScopeKind::Function);
                if variable.value == class_name.value {
                    // When accessing the method name in the constructor, return the class instead of the constructor
                    context.set("0");
                    context.set("self");
                } else {
                    context.set(&variable.value);
                }                
                let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                let block = translate_block(block, context)?;
                context.exit_scope();

                translated_methods.push(IRStatement {
                    kind: IRStatementKind::Method { name: variable.value, arguments: inner_arguements, block },
                    start: method.start,
                    end: method.end
                });
            }
        
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
                    context.enter_scope(ScopeKind::Function);
                    context.set(&variable.value);
                    let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.variable.value)).collect();
                    let block = translate_block(block, context)?;
                    context.exit_scope();

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