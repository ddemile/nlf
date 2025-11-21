use std::{collections::HashMap, vec};

use serde::Serialize;

use crate::parser::{Block, Expression, LiteralExpressionKind, Program, Statement, ValueHolder, VariableRef};

#[derive(Serialize, Debug, Clone)]
enum ScopeKind {
    Program,
    Call(usize),
    Loop,
    Conditional,
    Function,
    Block
}

#[derive(Serialize, Debug, Clone)]
struct SymbolTable {
    map: HashMap<String, usize>
}

impl SymbolTable {
    fn new() -> Self {
        SymbolTable { map: HashMap::new() }
    }

    fn get(&self, name: &str) -> Option<usize> {
        self.map.get(name).copied()
    }

    fn set(&mut self, name: &str) -> usize {
        let id = self.map.len();
        self.map.insert(name.to_string(), id);
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

            if let Some(v) = scope.symbol_table.get(name) {
                return Some(VariableRef { name: None, slot: v, depth });
            }

            depth += 1;
        }

        None
    }
    
    fn set(&mut self, name: &str) -> VariableRef {
        let scope = self.stack.last_mut().unwrap();

        let id = scope.symbol_table.set(name);

        // TOOD: refactor
        VariableRef { name: Some(name.to_owned()), slot: id, depth: 0 }
    }

    fn enter_scope(&mut self, scope: ScopeKind) {
        self.stack.push(Scope { kind: scope, slot_index: 0, symbol_table: SymbolTable::new() });
    }

    fn exit_scope(&mut self) {
        self.stack.pop();
    }
}

pub fn translate(program: Program) -> Program {
    let mut context = Context {
        stack: vec![Scope { kind: ScopeKind::Program, slot_index: 0, symbol_table: SymbolTable::new() }]
    };

    match translate_body(&program.body, &mut context) {
        Ok(statements) => Program { body: statements },
        Err(e) => {
            panic!("Translation error: {}", e);
        }
    }
}

fn translate_body(statements: &Vec<Statement>, context: &mut Context) -> Result<Vec<Statement>, String> {
    let mut inner_statements: Vec<Statement> = vec![];
    
    for statement in statements {
        inner_statements.push(translate_statement(statement.clone(), context)?);
    }

    Ok(inner_statements)
}

fn translate_statement(statement: Statement, context: &mut Context) -> Result<Statement, String> {
    match statement {
        Statement::Expression { expression } => {
            Ok(Statement::Expression { expression: translate_expression(expression, context)? })
        },
        Statement::If { condition, block, alternate } => {
            let condition = translate_expression(condition, context)?;

            context.enter_scope(ScopeKind::Conditional);
            let block = Block { statements: translate_body(&block.statements, context)? };
            context.exit_scope();


            let alternate = if let Some(alt) = alternate {
                Some(Box::new(translate_statement(*alt, context)?))
            } else {
                None
            };

            Ok(Statement::If { condition, block, alternate })
        },
        Statement::Function { name, arguments, statements } => {
            let var_ref = context.set(&name);
            context.enter_scope(ScopeKind::Function);
            context.set(&name);
            let inner_arguements: Vec<VariableRef> = arguments.iter().map(|arg| context.set(&arg.name)).collect();
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            Ok(Statement::FunctionIR { var_ref, arguments: inner_arguements, statements })
        },
        Statement::Block(Block { statements }) => {
            context.enter_scope(ScopeKind::Block);
            let block = Block { statements: translate_body(&statements, context)? };
            context.exit_scope();

            Ok(Statement::Block(block))
        }
        Statement::For { variable: Expression::Literal { value: ValueHolder::String(value), .. }, left, right, statements } => {
            let left = left.map(|left| translate_expression(left, context)).transpose()?;
            let right = right.map(|right| translate_expression(right, context)).transpose()?;

            context.enter_scope(ScopeKind::Loop);
            let var_ref = context.set(&value);
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            Ok(Statement::ForIR { variable: var_ref, left, right, statements })
        }
        Statement::While { condition, statements } => {
            
            context.enter_scope(ScopeKind::Loop);
            let condition = translate_expression(condition, context)?;
            let statements = translate_body(&statements, context)?;
            context.exit_scope();

            Ok(Statement::While { condition, statements })
        }
        Statement::Return { expression } => {
            Ok(Statement::Return { expression: translate_expression(expression, context)? })
        }
        Statement::Import { specifiers, source } => {
            let specifiers= specifiers.iter().map(|specifier| {
                let Expression::Literal { value: ValueHolder::String(name), ..  } = &specifier.local else {
                    unreachable!()
                };

                context.set(name)
            }).collect();

            Ok(Statement::ImportIR { specifiers, source })
        }
        Statement::Export { declaration } => {
            Ok(Statement::Export { declaration: Box::new(translate_statement(*declaration, context)?) })
        }
        _ => Ok(statement)
    }
}

fn translate_expression(expression: Expression, context: &mut Context) -> Result<Expression, String> {
    match &expression {
        Expression::Assignment { left, right, is_definition } => {
            let mut variable = *left.clone();

            // Find the innermost literal variable name
            loop {
                if matches!(&variable, Expression::Literal { .. }) {
                    break;
                }
                if let Expression::Member { object, .. } = &variable {
                    variable = object.as_ref().clone();
                } else {
                    break;
                }
            }

            let Expression::Literal { value: ValueHolder::String(name), ..  } = &variable else {
                return Err("Cannot access property on type other than a variable".to_string());
            };

            if *is_definition {
                context.set(name);
            }

            Ok(Expression::Assignment { left: Box::new(translate_expression(*left.clone(), context)?), right: Box::new(translate_expression(*right.clone(), context)?), is_definition: *is_definition })
        }
        Expression::Member { object, property } => {
            let object = translate_expression(*object.clone(), context)?;
            let property = translate_expression(*property.clone(), context)?;

            Ok(Expression::Member {
                object: Box::new(object),
                property: Box::new(property),
            })
        }
        Expression::Literal { r#type, value: ValueHolder::String(value) } => {
            if let LiteralExpressionKind::Variable = r#type {
                let variable_ref = context.get(value).ok_or("Not found")?;

                return Ok(Expression::Variable(variable_ref));
            }

            Ok(expression)
        }
        Expression::Binary { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            Ok(Expression::Binary {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            })
        }
        Expression::Logical { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            Ok(Expression::Logical {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            })
        }
        Expression::Equality { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            Ok(Expression::Equality {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            })
        }
        Expression::Relational { left, operator, right } => {
            let left = translate_expression(*left.clone(), context)?;
            let right = translate_expression(*right.clone(), context)?;

            Ok(Expression::Relational {
                left: Box::new(left),
                operator: operator.clone(),
                right: Box::new(right),
            })
        }
        Expression::Call { callee, arguments } => {
            let callee = match translate_expression(*callee.clone(), context) {
                Ok(expr) => expr,
                Err(_) => *callee.clone(),
            };
            
            let mut inner_arguments: Vec<Expression> = vec![];

            for arg in arguments {
                inner_arguments.push(translate_expression(arg.clone(), context)?);
            }

            Ok(Expression::Call {
                callee: Box::new(callee),
                arguments: inner_arguments,
            })
        }
        _ => Ok(expression)
    }
}