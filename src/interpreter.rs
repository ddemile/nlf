use std::{collections::HashMap, fmt};

use lazy_static::lazy_static;

use crate::{
    lexer::Token,
    parser::{Argument, Block, Expression, Function, LiteralExpressionKind, Program, Statement, ValueHolder},
};

#[derive(Debug)]
enum RuntimeError {
    InvalidType(String),
    Custom(String),
    VariableNotFound(String),
    VariableAlreadyDeclared(String)
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidType(value) => write!(f, "[InvalidType] {}", value),
            RuntimeError::Custom(value) => write!(f, "[CustomError] {}", value),
            RuntimeError::VariableNotFound(name) => write!(f, "[VariableNotFound] {}", name),
            RuntimeError::VariableAlreadyDeclared(name) => write!(f, "[VariableAlreadyDeclared] {}", name)
        }
    }
}

type RuntimeResult = Result<ValueHolder, RuntimeError>;

lazy_static! {
    static ref BUILT_IN_FUNCTIONS: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> ValueHolder + Send + Sync>> = {
        let mut m: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> ValueHolder + Send + Sync>> =
            HashMap::new();

        m.insert(
            String::from("println"),
            Box::new(|arguments| {

                let arguments: Vec<String> = arguments.iter().map(|argument| -> String {
                    match argument {
                        ValueHolder::String(value) => format!("{value}"),
                        ValueHolder::Float(value) => format!("{value}"),
                        ValueHolder::Int(value) => format!("{value}"),
                        ValueHolder::Bool(value) => format!("{value}"),
                        ValueHolder::Fn(Function { .. }) => format!("fn()"),
                        ValueHolder::Void => format!("Void")
                    }
                }).collect();

                println!("{}", arguments.join(" "));

                ValueHolder::Void
            }),
        );

        m.insert(
            String::from("pow"),
            Box::new(|arguments| {
                let a = match arguments.get(0).unwrap() {
                    ValueHolder::Float(f) => f,
                    _ => panic!("Expected float value"),
                };
                let b = match arguments.get(1).unwrap() {
                    ValueHolder::Float(f) => f,
                    _ => panic!("Expected float value"),
                };

                ValueHolder::Float(a.powf(*b))
            }),
        );

        m
    };
}

struct ProgramContext {
    pub environment: Environment,
}

#[derive(Debug, Clone)]
enum ScopeKind {
    Program,
    Call(i32),
    Loop,
    Regular
}

#[derive(Debug, Clone)]
struct Scope {
    kind: ScopeKind,
    variables: HashMap<String, ValueHolder>,
    isolated: bool,
    interrupted: Option<ValueHolder>
}

struct Environment {
    scopes: Vec<Scope>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                kind: ScopeKind::Program,
                variables: HashMap::new(),
                isolated: false,
                interrupted: None
            }],
        }
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope {
            kind,
            variables: HashMap::new(),
            isolated: false,
            interrupted: None
        });
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop().expect("No scope to exit");
    }

    pub fn set(&mut self, name: String, value: ValueHolder, define: bool) -> Result<(), RuntimeError> {
        if !define {
            for scope in self.scopes.iter_mut().rev() {
                if scope.variables.contains_key(&name) {
                    scope.variables.insert(name.clone(), value.clone());
                    return Ok(())
                }

                if let ScopeKind::Call(_) = scope.kind {
                    break;
                }
            }
            return Err(RuntimeError::VariableNotFound(name))
        } else if let Some(scope) = self.scopes.last_mut() {
            if scope.variables.contains_key(&name) {
                return Err(RuntimeError::VariableAlreadyDeclared(name))
            }
            
            scope.variables.insert(name, value);
            return Ok(())
        }
        unreachable!()
    }

    pub fn get(&self, name: &str) -> Option<&ValueHolder> {
        let mut scopes = self.scopes.iter().rev();

        while let Some(scope) = scopes.next() {
            if let ScopeKind::Call(idx) = scope.kind {
                scopes = self.scopes[0..((idx + 1) as usize)].iter().rev();
            }

            if let Some(v) = scope.variables.get(name) {
                return Some(v);
            }
            if scope.isolated {
                break; // stop searching outer scopes
            }
        }
        None
    }
}

pub fn interpret(program: Program) {
    let environment = Environment::new();

    let mut context = ProgramContext { environment };

    eval_body(&program.body, &mut context).unwrap();
}

fn define_functions(statements: &Vec<Statement>, context: &mut ProgramContext) -> Result<(), RuntimeError> {
    for statement in statements.iter().filter(|statement| matches!(*statement, Statement::Function { .. })) {
        let Statement::Function { name, arguments, statements } = statement else {
            panic!()
        };

        context.environment.set(name.clone(), ValueHolder::Fn(Function {
            arguments: arguments.to_vec(),
            statements: statements.to_vec()
        }), true)?
    }

    Ok(())
}

fn eval_body(statements: &Vec<Statement>, context: &mut ProgramContext) -> RuntimeResult {
    define_functions(statements, context)?;

    for statement in statements {
        let value = eval_statement(statement.clone(), context)?;
        if let Some(value) = &context.environment.scopes.last().unwrap().interrupted {
            return Ok(value.clone());
        }   

        for scope in context.environment.scopes.iter_mut().rev() {
            match (statement.clone(), scope.kind.clone(), ) {
                (Statement::Return { .. }, ScopeKind::Call(_)) => {
                    scope.interrupted = Some(value.clone());

                    return Ok(value);
                },
                (_, ScopeKind::Call(_)) => {
                    break;
                },
                (Statement::Break, ScopeKind::Loop) => {
                    scope.interrupted = Some(ValueHolder::Void);

                    return Ok(value);
                },
                _ => ()
            }
        }
    }

    Ok(ValueHolder::Void)
}

fn eval_statement(statement: Statement, context: &mut ProgramContext) -> RuntimeResult {
    match statement {
        Statement::Expression { expression } => eval_expr(expression, context),
        Statement::If {
            condition,
            block,
            alternate
        } => eval_if(condition, block, alternate, context),
        Statement::For {
            variable,
            left,
            right,
            statements,
        } => eval_for(variable, left, right, statements, context),
        Statement::Function { name, arguments, statements } => eval_function(name, arguments, statements),
        Statement::Return { expression } => eval_expr(expression, context),
        Statement::Break => Ok(ValueHolder::Void),
        Statement::Block(_) => Ok(ValueHolder::Void)
    }
}

fn eval_function(_name: String, _arguments: Vec<Argument>, _statements: Vec<Statement>) -> RuntimeResult {
    

    Ok(ValueHolder::Void)
}

fn eval_for(
    variable: Expression,
    left: Option<Expression>,
    right: Option<Expression>,
    statements: Vec<Statement>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let Expression::Literal { value, ..} = variable else {
        return Err(RuntimeError::InvalidType("For loop variable should be a literal".to_string()))
    };

    let ValueHolder::String(variable_identifer) = value else {
        return Err(RuntimeError::InvalidType("For loop variable should be a variable".to_string()))
    };

    let mut iter: Box<dyn Iterator<Item=i32>> = Box::new(0..);

    match (&left, &right) {
        (Some(_), Some(_)) => {
            let left: i32 = eval_expr(left.unwrap(), context)?.into();
            let right: i32 = eval_expr(right.unwrap(), context)?.into();

            iter = if left < right { Box::new(left..right) } else { Box::new(((right + 1)..(left + 1)).rev()) };
        },
        (Some(_), None) => {
            let left: i32 = eval_expr(left.unwrap(), context)?.into();

            iter = Box::new(left..)
        },
        (None, Some(_)) => {
            let right: i32 = eval_expr(right.unwrap(), context)?.into();

            iter = Box::new(0..right)
        },
        _ => (),
    }

    for i in iter {
        context.environment.enter_scope(ScopeKind::Loop);
        context.environment.set(variable_identifer.clone(), ValueHolder::Int(i), true)?;
        eval_body(&statements, context)?;
        let broken = context.environment.scopes.last().unwrap().interrupted.is_some();
        context.environment.exit_scope();

        if broken {
            break;
        }
    }

    Ok(ValueHolder::Void)
}

fn eval_if(
    condition: Expression,
    block: Block,
    alternate: Option<Box<Statement>>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let cond: bool = eval_expr(condition, context)?.into();
    if cond {
        context.environment.enter_scope(ScopeKind::Regular);
        eval_body(&block.statements, context)?;
        context.environment.exit_scope();
    } else if let Some(box Statement::If { condition, block, alternate }) = alternate {
        return eval_if(condition, block, alternate, context);
    } else if let Some(box Statement::Block(Block { statements })) = alternate {
        return eval_body(&statements, context);
    }
    Ok(ValueHolder::Void)
}

fn eval_expr(expr: Expression, context: &mut ProgramContext) -> RuntimeResult {
    return match expr {
        Expression::Binary {
            left,
            operator,
            right,
        } => eval_binary(left, operator, right, context),
        Expression::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                return Ok(value);
            } else {
                if let ValueHolder::String(value) = value {
                    return Ok(context
                        .environment
                        .get(&value)
                        .ok_or(RuntimeError::Custom(format!("Variable '{value}' not found")))?
                        .clone());
                }

                panic!("Variable error")
            };
        }
        Expression::Assignment { left, right, is_definition } => {
            if let Expression::Literal { r#type: _, value } = *left {
                if let ValueHolder::String(value) = value {
                    let expr = eval_expr(*right, context)?;
                    let _ = context.environment.set(value, expr, is_definition)?;

                    return Ok(ValueHolder::Void);
                }
            }

            panic!("Expected variable for assignment")
        }
        Expression::Call { name, arguments } => eval_call(name, arguments, context),
        Expression::Equality {
            left,
            operator,
            right,
        } => eval_equality(left, operator, right, context),
        Expression::Relational {
            left,
            operator,
            right,
        } => eval_relational(left, operator, right, context),
        Expression::Logical {
            left,
            operator,
            right,
        } => eval_logical(left, operator, right, context),
    };
}

fn eval_binary(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let left_val = match eval_expr(*left, context) {
        Ok(ValueHolder::Float(f)) => f,
        Ok(ValueHolder::Int(f)) => f.into(),
        _ => return Err(RuntimeError::InvalidType("Expected float value for binary operation".to_string())),
    };
    let right_val = match eval_expr(*right, context) {
        Ok(ValueHolder::Float(f)) => f,
        Ok(ValueHolder::Int(f)) => f.into(),
        _ => return Err(RuntimeError::InvalidType("Expected float value for binary operation".to_string())),
    };

    match operator {
        Token::Plus => Ok(ValueHolder::Float(left_val + right_val)),
        Token::Minus => Ok(ValueHolder::Float(left_val - right_val)),
        Token::Asterisk => Ok(ValueHolder::Float(left_val * right_val)),
        Token::Slash => Ok(ValueHolder::Float(left_val / right_val)),
        _ => panic!("Invalid binary operator"),
    }
}

fn eval_call(
    name: String,
    call_arguments: Vec<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let func = BUILT_IN_FUNCTIONS.get(&name);

    if let Some(func) = func {
        let arguments: Vec<ValueHolder> = call_arguments
            .into_iter()
            .map(|arg| eval_expr(arg, context))
            .collect::<Result<Vec<_>, RuntimeError>>()?;

        return Ok(func(arguments));
    };
    
    let func = context.environment.get(&name).cloned();

    if let Some(ValueHolder::Fn(Function { arguments, statements })) = func {
        let scope_position = context.environment.scopes.iter().position(|scope| scope.variables.contains_key(&name)).unwrap();

        if arguments.len() != call_arguments.len() {
            return Err(RuntimeError::Custom("Invalid number of args".to_string()))
        }

        // Evaluate all arguments before entering the scope to avoid multiple mutable borrows
        let evaluated_args: Vec<ValueHolder> = call_arguments
            .into_iter()
            .map(|arg| eval_expr(arg, context))
        .collect::<Result<Vec<_>, RuntimeError>>()?;

        context.environment.enter_scope(ScopeKind::Call(scope_position as i32));
        for (argument, value) in arguments.iter().zip(evaluated_args.iter()) {
            context.environment.set(
                argument.name.clone(),
                value.clone(),
                true
            )?;
        }
        let return_value = eval_body(&statements, context);
        context.environment.exit_scope();
        return return_value;
    }

    panic!("Tried to call invalid function : {}", name);
}

fn eval_equality(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let left = eval_expr(*left, context)?;
    let right = eval_expr(*right, context)?;

    match operator {
        Token::EQ => Ok(ValueHolder::Bool(left == right)),
        Token::NE => Ok(ValueHolder::Bool(left != right)),
        _ => Err(RuntimeError::Custom("Invalid equality operator".to_string())),
    }
}

fn eval_relational(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let left = eval_expr(*left, context)?;
    let right = eval_expr(*right, context)?;

    Ok(ValueHolder::Bool(
        match operator {
            Token::GT => left > right,
            Token::GTE => left >= right,
            Token::LT => left < right,
            Token::LTE => left <= right,
            _ => unreachable!(),
        }
    ))
}

fn eval_logical(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let left = eval_expr(*left, context)?;
    let right = eval_expr(*right, context)?;

    match operator {
        Token::And => Ok(ValueHolder::Bool(left.into() && right.into())),
        Token::Or => Ok(ValueHolder::Bool(left.into() || right.into())),
        _ => Err(RuntimeError::Custom("Invalid relational operator".to_string())),
    }
}
