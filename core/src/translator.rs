use std::{collections::HashMap, vec};

use serde::Serialize;

use crate::{errors::{LanguageError, LanguageErrorTrait, LanguageResult}, parser::{Block, Expression, ExpressionKind, LiteralExpressionKind, Program, Statement, StatementKind, ValueHolder, VariableRef}};

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

pub fn translate(program: Program) -> LanguageResult<Program> {
    let mut context = Context {
        stack: vec![Scope { kind: ScopeKind::Program, slot_index: 0, symbol_table: SymbolTable::new() }]
    };

    translate_body(&program.body, &mut context).map(|statements| Program { body: statements })
}

fn translate_body(statements: &Vec<Statement>, context: &mut Context) -> LanguageResult<Vec<Statement>> {
    let mut inner_statements: Vec<Statement> = vec![];
    
    for statement in statements {
        inner_statements.push(translate_statement(statement.clone(), context)?);
    }

    Ok(inner_statements)
}

fn translate_statement(statement: Statement, context: &mut Context) -> LanguageResult<Statement> {
    let mut new_statement = statement.clone();
    new_statement.kind = match statement.kind {
        StatementKind::Expression { expression } => {
            StatementKind::Expression { expression: translate_expression(expression, context)? }
        },
        StatementKind::If { condition, block, alternate } => {
            let condition = translate_expression(condition, context)?;

            context.enter_scope(ScopeKind::Conditional);
            let block = Block { statements: translate_body(&block.statements, context)?, start: block.start, end: block.end };
            context.exit_scope();

            let alternate = if let Some(alt) = alternate {
                Some(Box::new(translate_statement(*alt, context)?))
            } else {
                None
            };

            StatementKind::If { condition, block, alternate }
        },
        StatementKind::Function { name, arguments, statements } => {
            let var_ref = context.set(&name);
            context.enter_scope(ScopeKind::Function);
            context.set(&name);
            let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.name)).collect();
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            StatementKind::FunctionIR { var_ref, arguments: inner_arguements, statements }
        },
        StatementKind::Block(Block { statements, start, end }) => {
            context.enter_scope(ScopeKind::Block);
            let block = Block { statements: translate_body(&statements, context)?, start, end };
            context.exit_scope();

            StatementKind::Block(block)
        }
        StatementKind::For { variable: Expression { kind: ExpressionKind::Literal { value: ValueHolder::String(value), .. }, .. }, left, right, statements } => {
            let left = left.map(|left| translate_expression(left, context)).transpose()?;
            let right = right.map(|right| translate_expression(right, context)).transpose()?;

            context.enter_scope(ScopeKind::Loop);
            let var_ref = context.set(&value);
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            StatementKind::ForIR { variable: var_ref, left, right, statements }
        }
        StatementKind::While { condition, statements } => {
            
            context.enter_scope(ScopeKind::Loop);
            let condition = translate_expression(condition, context)?;
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            StatementKind::While { condition, statements }
        }
        StatementKind::Return { expression } => {
            StatementKind::Return { expression: translate_expression(expression, context)? }
        }
        StatementKind::Import { specifiers, source } => {
            let specifiers= specifiers.iter().map(|specifier| {
                let ExpressionKind::Literal { value: ValueHolder::String(name), ..  } = &specifier.local.kind else {
                    unreachable!()
                };

                context.set(name)
            }).collect();

            StatementKind::ImportIR { specifiers, source }
        }
        StatementKind::Export { declaration } => {
            StatementKind::Export { declaration: Box::new(translate_statement(*declaration, context)?) }
        }
        StatementKind::Class { name: class_name,  methods, fields } => {
            let var_ref = context.set(&class_name);

            let mut translated_methods = vec![];
            for mut method in methods {
                let StatementKind::Function { name, arguments, statements } = method.kind else {
                    unreachable!()
                };

                context.enter_scope(ScopeKind::Function);
                if name == class_name {
                    // When accessing the method name in the constructor, return the class instead of the constructor
                    context.set("0");
                } else {
                    context.set(&name);
                }
                context.set("self");
                let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.name)).collect();
                let statements = translate_body(&statements, context)?;
                context.exit_scope();

                method.kind = StatementKind::Method { name, arguments: inner_arguements, statements };

                translated_methods.push(method);
            }
        
            StatementKind::ClassIR { var_ref, methods: translated_methods, fields }
        }
        _ => statement.kind
    };
    Ok(new_statement)
}

fn translate_expression(mut expression: Expression, context: &mut Context) -> LanguageResult<Expression> {
    expression.kind = match &expression.kind {
        ExpressionKind::Assignment { left, operator, right, is_definition } => {
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

            let ExpressionKind::Literal { value: ValueHolder::String(name), ..  } = &variable.kind else {
                return Err(LanguageError::from(TranslatorError::TODO("Cannot access property on type other than a variable".to_string())));
            };

            if *is_definition {
                context.set(name);
            }

            ExpressionKind::Assignment { left: Box::new(translate_expression(*left.clone(), context)?), operator: operator.clone(), right: Box::new(translate_expression(*right.clone(), context)?), is_definition: *is_definition }
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

                    let variable_ref = context.get(value).ok_or(LanguageError::from(TranslatorError::UnknownVariable(value.clone())))?;

                    return Ok(ExpressionKind::Variable(variable_ref).into_expression(expression.start, expression.end));
                }
                LiteralExpressionKind::Function(statement) => {
                    let box StatementKind::Function { name, arguments, statements } = statement.clone() else {
                        panic!()
                    };

                    let var_ref = context.set(&name);
                    context.enter_scope(ScopeKind::Function);
                    context.set(&name);
                    let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.name)).collect();
                    let statements = translate_body(&statements, context)?;
                    context.exit_scope();

                    return Ok(ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(Box::new(StatementKind::FunctionIR {
                        var_ref,
                        arguments: inner_arguements,
                        statements
                    })), value: value.clone() }.into_expression(expression.start, expression.end))
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