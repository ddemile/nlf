use std::{collections::HashMap, fmt::format, ops::Index};

use lazy_static::lazy_static;

use crate::{
    lexer::Token,
    parser::{Argument, Expression, Function, LiteralExpressionKind, Program, Statement, ValueHolder},
};

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
                        _ => panic!("[Invalid type]"), 
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

struct Scope {
    variables: HashMap<String, ValueHolder>,
    isolated: bool,
}

struct Environment {
    scopes: Vec<Scope>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                variables: HashMap::new(),
                isolated: false,
            }],
        }
    }

    pub fn enter_scope(&mut self, isolated: bool) {
        self.scopes.push(Scope {
            variables: HashMap::new(),
            isolated,
        });
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop().expect("No scope to exit");
    }

    pub fn set(&mut self, name: String, value: ValueHolder) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.variables.insert(name, value);
        }
    }

    pub fn get(&self, name: &str) -> Option<&ValueHolder> {
        for scope in self.scopes.iter().rev() {
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

    eval_body(&program.body, &mut context);
}

fn eval_body(statements: &Vec<Statement>, context: &mut ProgramContext) {
    statements.iter().filter(|statement| matches!(*statement, Statement::Function { .. })).for_each(|statement| {
        let Statement::Function { name, arguments, statements } = statement else {
            panic!()
        };

        context.environment.set(name.clone(), ValueHolder::Fn(Function {
            arguments: arguments.to_vec(),
            statements: statements.to_vec()
        }));
    });
    for statement in statements {
        eval_statement(statement.clone(), context);
    }
}

fn eval_statement(statement: Statement, context: &mut ProgramContext) -> ValueHolder {
    match statement {
        Statement::Expression { expression } => eval_expr(expression, context),
        Statement::If {
            condition,
            statements,
        } => eval_if(condition, statements, context),
        Statement::For {
            variable,
            left,
            right,
            statements,
        } => eval_for(variable, left, right, statements, context),
        Statement::Function { name, arguments, statements } => eval_function(name, arguments, statements)
    }
}

fn eval_function(name: String, arguments: Vec<Argument>, statements: Vec<Statement>) -> ValueHolder {
    

    ValueHolder::Void
}

fn eval_for(
    variable: Expression,
    left: Option<Expression>,
    right: Option<Expression>,
    statements: Vec<Statement>,
    context: &mut ProgramContext,
) -> ValueHolder {
    let Expression::Literal { value, ..} = variable else {
        panic!("For loop variable should be a literal");
    };

    let ValueHolder::String(variable_identifer) = value else {
        panic!("For loop variable should be a variable")
    };

    let mut iter: Box<dyn Iterator<Item=i32>> = Box::new(0..);

    match (&left, &right) {
        (Some(Expression::Literal { .. }), Some(Expression::Literal { .. })) => {
            let left: i32 = eval_expr(left.unwrap(), context).into();
            let right: i32 = eval_expr(right.unwrap(), context).into();

            iter = if left < right { Box::new(left..right) } else { Box::new(((right + 1)..(left + 1)).rev()) };


        },
        (Some(Expression::Literal { .. }), None) => {
            let left: i32 = eval_expr(left.unwrap(), context).into();

            iter = Box::new(left..)
        },
        (None, Some(Expression::Literal { .. })) => {
            let right: i32 = eval_expr(right.unwrap(), context).into();

            iter = Box::new(0..right)
        },
        _ => (),
    }

    for i in iter {
        context.environment.enter_scope(false);
        context.environment.set(variable_identifer.clone(), ValueHolder::Int(i));
        eval_body(&statements, context);
        context.environment.exit_scope();
    }

    ValueHolder::Void
}

fn eval_if(
    condition: Expression,
    statements: Vec<Statement>,
    context: &mut ProgramContext,
) -> ValueHolder {
    let cond: bool = eval_expr(condition, context).into();
    if cond {
        context.environment.enter_scope(false);
        eval_body(&statements, context);
        context.environment.exit_scope();
    }
    ValueHolder::Void
}

fn eval_expr(expr: Expression, context: &mut ProgramContext) -> ValueHolder {
    return match expr {
        Expression::Binary {
            left,
            operator,
            right,
        } => ValueHolder::Float(eval_binary(left, operator, right, context)),
        Expression::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                return value;
            } else {
                if let ValueHolder::String(value) = value {
                    return context
                        .environment
                        .get(&value)
                        .expect(&format!("Unknown variable {}", value))
                        .clone();
                }

                panic!("Variable error")
            };
        }
        Expression::Assignment { left, right } => {
            if let Expression::Literal { r#type: _, value } = *left {
                if let ValueHolder::String(value) = value {
                    let expr = eval_expr(*right, context);
                    let _ = context.environment.set(value, expr);

                    return ValueHolder::Void;
                }
            }

            panic!("Expected variable for assignment")
        }
        Expression::Call { name, arguments } => eval_call(name, arguments, context),
        Expression::Equality {
            left,
            operator,
            right,
        } => ValueHolder::Bool(eval_equality(left, operator, right, context)),
        Expression::Relational {
            left,
            operator,
            right,
        } => ValueHolder::Bool(eval_relational(left, operator, right, context)),
        Expression::Logical {
            left,
            operator,
            right,
        } => ValueHolder::Bool(eval_logical(left, operator, right, context)),
        _ => panic!("Invalid expression"),
    };
}

fn eval_binary(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> f64 {
    let left_val = match eval_expr(*left, context) {
        ValueHolder::Float(f) => f,
        ValueHolder::Int(f) => f.into(),
        _ => panic!("Expected float value"),
    };
    let right_val = match eval_expr(*right, context) {
        ValueHolder::Float(f) => f,
        ValueHolder::Int(f) => f.into(),
        _ => panic!("Expected float value"),
    };

    match operator {
        Token::Plus => left_val + right_val,
        Token::Minus => left_val - right_val,
        Token::Asterisk => left_val * right_val,
        Token::Slash => left_val / right_val,
        _ => panic!("Invalid binary operator"),
    }
}

fn eval_call(
    name: String,
    call_arguments: Vec<Expression>,
    context: &mut ProgramContext,
) -> ValueHolder {
    let func = BUILT_IN_FUNCTIONS.get(&name);

    if let Some(func) = func {
        let arguments: Vec<ValueHolder> = call_arguments
            .into_iter()
            .map(|arg| eval_expr(arg, context))
            .collect();

        return func(arguments);
    };
    
    let func = context.environment.get(&name).cloned();

    if let Some(ValueHolder::Fn(Function { arguments, statements })) = func {
        if arguments.len() != call_arguments.len() {
            panic!("Invalid number of args");
        }

        // Evaluate all arguments before entering the scope to avoid multiple mutable borrows
        let evaluated_args: Vec<ValueHolder> = call_arguments
            .into_iter()
            .map(|arg| eval_expr(arg, context))
            .collect();

        context.environment.enter_scope(false);
        for (argument, value) in arguments.iter().zip(evaluated_args.iter()) {
            context.environment.set(
                argument.name.clone(),
                value.clone()
            );
        }
        eval_body(&statements, context);
        context.environment.exit_scope();
        return ValueHolder::Void;
    }

    panic!("Tried to call invalid function : {}", name);
}

fn eval_equality(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> bool {
    let left = eval_expr(*left, context);
    let right = eval_expr(*right, context);

    match operator {
        Token::EQ => left == right,
        Token::NE => left != right,
        _ => panic!("Invalid equality operator"),
    }
}

fn eval_relational(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> bool {
    let left = eval_expr(*left, context);
    let right = eval_expr(*right, context);

    match operator {
        Token::GT => left > right,
        Token::GTE => left >= right,
        Token::LT => left < right,
        Token::LTE => left <= right,
        _ => panic!("Invalid relational operator"),
    }
}

fn eval_logical(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ProgramContext,
) -> bool {
    let left = eval_expr(*left, context);
    let right = eval_expr(*right, context);

    match operator {
        Token::And => left.into() && right.into(),
        Token::Or => left.into() || right.into(),
        _ => panic!("Invalid relational operator"),
    }
}
