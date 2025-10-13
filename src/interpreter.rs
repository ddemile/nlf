use core::panic;
use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc, time::{SystemTime, UNIX_EPOCH}};

use indexmap::IndexSet;
use lazy_static::lazy_static;

use crate::{
    lexer::Token,
    parser::{
        Argument, Block, Expression, Function, LiteralExpressionKind, ObjectRef, Program,
        Statement, ValueHolder, VariableRef,
    },
};

#[derive(Debug)]
enum RuntimeError {
    InvalidType(String),
    Custom(String),
    VariableNotFound(String),
    VariableAlreadyDeclared(String),
    NoSuchProperty(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidType(value) => write!(f, "[InvalidType] {}", value),
            RuntimeError::Custom(value) => write!(f, "[CustomError] {}", value),
            RuntimeError::VariableNotFound(name) => write!(f, "[VariableNotFound] {}", name),
            RuntimeError::VariableAlreadyDeclared(name) => {
                write!(f, "[VariableAlreadyDeclared] {}", name)
            }
            RuntimeError::NoSuchProperty(name) => {
                write!(f, "[NoSuchProperty] {}", name)
            }
        }
    }
}

type RuntimeResult = Result<ValueHolder, RuntimeError>;

lazy_static! {
    static ref BUILT_IN_FUNCTIONS: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> ValueHolder + Send + Sync>> = {
        let mut m: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> ValueHolder + Send + Sync>> =
            HashMap::new();

        m.insert(
            String::from("print"),
            Box::new(|arguments| {
                let arguments: Vec<String> = arguments
                    .iter()
                    .map(|argument| -> String {
                        match argument {
                            ValueHolder::String(value) => format!("{value}"),
                            ValueHolder::Float(value) => format!("{value}"),
                            ValueHolder::Int(value) => format!("{value}"),
                            ValueHolder::Bool(value) => format!("{value}"),
                            ValueHolder::Fn(Function { .. }) => format!("fn()"),
                            ValueHolder::Object(_) => format!("ObjectRef"),
                            ValueHolder::Void => format!("Void"),
                        }
                    })
                    .collect();

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

        m.insert(
            String::from("now"),
            Box::new(|_| {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("time should go forward");
                ValueHolder::Float(since_the_epoch.as_millis_f64())
            }),
        );

        m
    };
}

struct ProgramContext {
    pub environment: Environment,
    pub schemas: Vec<Schema>,
    pub store: HashMap<usize, Object>,
}

#[derive(Clone, Debug)]
struct Schema {
    id: usize,
    keys: IndexSet<String>,
}

struct Object {
    schema_id: usize,
    values: Vec<ValueHolder>,
}

fn get_schema(keys: IndexSet<String>, context: &mut ProgramContext) -> Schema {
    context
        .schemas
        .iter()
        .find(|s| s.keys == keys)
        .cloned()
        .unwrap_or_else(|| {
            let schema = Schema {
                id: context.schemas.len(),
                keys,
            };
            context.schemas.push(schema.clone());
            schema
        })
}

impl ObjectRef {
    pub(self) fn new(entries: HashMap<String, ValueHolder>, context: &mut ProgramContext) -> Self {
        let keys: IndexSet<String> = entries.keys().cloned().collect();

        let schema = get_schema(keys, context);

        let object = Object {
            schema_id: schema.id,
            values: entries.values().cloned().collect(),
        };

        let object_id = context.store.len() + 1;

        context.store.insert(object_id, object);

        ObjectRef { object_id }
    }

    pub(self) fn get(self, property: String, context: &ProgramContext) -> RuntimeResult {
        let object = context
            .store
            .get(&self.object_id)
            .expect("Object not found");

        let schema = context
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let index = schema.keys.iter().position(|key| *key == property);

        if let Some(index) = index {
            return Ok(object.values[index].clone());
        }

        Err(RuntimeError::NoSuchProperty(property))
    }

    pub(self) fn set(&self, property: String, value: ValueHolder, context: &mut ProgramContext) {
        let object = context
            .store
            .get(&self.object_id)
            .expect("Object not found");

        let schema = context
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let mut keys = schema.keys.clone();
        let mut values = object.values.clone();

        keys.insert(property.clone());

        if let Some(index) = keys.iter().position(|key| *key == property) {
            if index >= values.len() {
                values.push(value);
            } else {
                values[index] = value;
            }
        }

        let schema = get_schema(keys, context);

        let object = Object {
            schema_id: schema.id,
            values,
        };

        context.store.insert(self.object_id, object);
    }
}

#[derive(Debug, Clone)]
enum ScopeKind {
    Program,
    Call(i32),
    Loop,
    Regular,
}

#[derive(Debug, Clone)]
struct Scope {
    kind: ScopeKind,
    slots: Vec<ValueHolder>,
    isolated: bool,
    interrupted: Option<ValueHolder>,
    parent: Option<usize>
}

struct Environment {
    scopes: Vec<Scope>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                kind: ScopeKind::Program,
                slots: vec![],
                isolated: false,
                interrupted: None,
                parent: None
            }],
        }
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope {
            kind,
            slots: vec![],
            isolated: false,
            interrupted: None,
            parent: Some(self.scopes.len() - 1)
        });
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop().expect("No scope to exit");
    }

pub fn set(
        &mut self,
        var_ref: VariableRef,
        value: ValueHolder,
        define: bool,
    ) -> Result<(), RuntimeError> {
        if !define {
            // Traverse up the parent chain to find the correct scope
            let mut scope_idx = self.scopes.len() - 1;
            for _ in 0..var_ref.depth {
                let scope = &self.scopes[scope_idx];
                scope_idx = scope.parent.ok_or(RuntimeError::VariableNotFound(format!(
                    "Parent scope not found for slot {} at depth {}",
                    var_ref.slot, var_ref.depth
                )))?;
            }
            self.scopes[scope_idx]
                .slots
                .get_mut(var_ref.slot)
                .map(|v| *v = value)
                .ok_or(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    var_ref.slot, var_ref.depth
                )))?;
            return Ok(());
        } else if let Some(scope) = self.scopes.last_mut() {
            if scope.slots.len() <= var_ref.slot {
                scope.slots.resize(var_ref.slot + 1, ValueHolder::Void);
            }
            scope.slots[var_ref.slot] = value;
            return Ok(());
        }
        unreachable!()
    }

    pub fn get(&self, var_ref: VariableRef) -> Option<&ValueHolder> {
        // Traverse up the parent chain to find the correct scope
        let mut scope_idx = self.scopes.len() - 1;
        for _ in 0..var_ref.depth {
            let scope = &self.scopes[scope_idx];
            scope_idx = scope.parent?;
        }
        self.scopes[scope_idx].slots.get(var_ref.slot)
    }
}

pub fn interpret(program: Program) {
    let environment = Environment::new();

    let mut context = ProgramContext {
        environment,
        schemas: vec![],
        store: HashMap::new(),
    };

    match eval_body(&program.body, &mut context) {
        Ok(_) => (),
        Err(e) => {
            for scope in context.environment.scopes.iter() {
                // println!("{:?}", scope);
            }
            println!("Runtime error: {}", e)
        }
    }
}

fn define_functions(
    statements: &Vec<Statement>,
    context: &mut ProgramContext,
) -> Result<(), RuntimeError> {
    for statement in statements
        .iter()
        .filter(|statement| matches!(*statement, Statement::FunctionIR { .. }))
    {
        let Statement::FunctionIR {
            var_ref,
            arguments,
            statements,
        } = statement
        else {
            panic!()
        };

        context.environment.set(
            var_ref.clone(),
            ValueHolder::Fn(Function {
                arguments: arguments.to_vec(),
                statements: statements.to_vec(),
                scope_position: context.environment.scopes.len() - 1
            }),
            true,
        )?
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
            match (statement.clone(), scope.kind.clone()) {
                (Statement::Return { .. }, ScopeKind::Call(_)) => {
                    scope.interrupted = Some(value.clone());

                    return Ok(value);
                }
                (_, ScopeKind::Call(_)) => {
                    break;
                }
                (Statement::Break, ScopeKind::Loop) => {
                    scope.interrupted = Some(ValueHolder::Void);

                    return Ok(value);
                }
                _ => (),
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
            alternate,
        } => eval_if(condition, block, alternate, context),
        Statement::ForIR {
            variable,
            left,
            right,
            statements,
        } => eval_for(variable, left, right, statements, context),
        Statement::FunctionIR {
            var_ref,
            arguments,
            statements,
        } => eval_function(var_ref, arguments, statements),
        Statement::Return { expression } => eval_expr(expression, context),
        Statement::Break => Ok(ValueHolder::Void),
        Statement::Block(_) => Ok(ValueHolder::Void),
        _ => panic!("Invalid statement"),
    }
}

fn eval_function(
    _var_ref: VariableRef,
    _arguments: Vec<VariableRef>,
    _statements: Vec<Statement>,
) -> RuntimeResult {
    Ok(ValueHolder::Void)
}

fn eval_for(
    variable: VariableRef,
    left: Option<Expression>,
    right: Option<Expression>,
    statements: Vec<Statement>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let mut iter: Box<dyn Iterator<Item = i32>> = Box::new(0..);

    match (&left, &right) {
        (Some(_), Some(_)) => {
            let left: i32 = eval_expr(left.unwrap(), context)?.into();
            let right: i32 = eval_expr(right.unwrap(), context)?.into();

            iter = if left < right {
                Box::new(left..right)
            } else {
                Box::new(((right + 1)..(left + 1)).rev())
            };
        }
        (Some(_), None) => {
            let left: i32 = eval_expr(left.unwrap(), context)?.into();

            iter = Box::new(left..)
        }
        (None, Some(_)) => {
            let right: i32 = eval_expr(right.unwrap(), context)?.into();

            iter = Box::new(0..right)
        }
        _ => (),
    }

    context.environment.enter_scope(ScopeKind::Loop);
    context
        .environment
        .set(variable.clone(), ValueHolder::Int(0), true)?;

    for i in iter {
        // Faster than environment.set in this context
        let scope: &mut &mut Scope = &mut context.environment.scopes.last_mut().unwrap();
        scope.slots[variable.slot] = ValueHolder::Int(i);

        context.environment.enter_scope(ScopeKind::Regular);

        eval_body(&statements, context)?;
        let broken = context
            .environment
            .scopes
            .last()
            .unwrap()
            .interrupted
            .is_some();
        context.environment.exit_scope();

        if broken {
            break;
        }
    }
    context.environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_if(
    condition: Expression,
    block: Block,
    alternate: Option<Box<Statement>>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    let cond: bool = eval_expr(condition, context)?.into();
    context.environment.enter_scope(ScopeKind::Regular);
    if cond {
        eval_body(&block.statements, context)?;
    } else if let Some(box Statement::If {
        condition,
        block,
        alternate,
    }) = alternate
    {
        eval_if(condition, block, alternate, context)?;
    } else if let Some(box Statement::Block(Block { statements })) = alternate {
        eval_body(&statements, context)?;
    }
    context.environment.exit_scope();
    Ok(ValueHolder::Void)
}

fn eval_expr(expr: Expression, context: &mut ProgramContext) -> RuntimeResult {
    return match expr {
        Expression::Binary {
            left,
            operator,
            right,
        } => eval_binary(left, operator, right, context),
        Expression::Member { object, property } => {
            let value = eval_expr(*object, context)?;

            let Expression::Literal {
                r#type: LiteralExpressionKind::Variable,
                value: ValueHolder::String(property),
            } = *property
            else {
                unreachable!();
            };

            let ValueHolder::Object(object) = value else {
                return Err(RuntimeError::InvalidType(
                    "Cannot access property on type other than Object".to_string(),
                ));
            };

            object.get(property, context)
        }
        Expression::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                return Ok(value);
            } else if let LiteralExpressionKind::Object(entries) = r#type {
                let entries: HashMap<String, ValueHolder> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), eval_expr(v.clone(), context).unwrap()))
                    .collect();
                let object_ref = ObjectRef::new(entries, context);
                return Ok(ValueHolder::Object(object_ref));
            } else {
                panic!("Variable error")
            };
        }
        Expression::Variable(variable) => {
            context.environment
                .get(variable.clone())
                .cloned()
                .ok_or(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    variable.slot, variable.depth
                )))
        }
        Expression::Assignment {
            left,
            right,
            is_definition,
        } => {
            if let Expression::Variable(var_ref) = *left {
                let expr = eval_expr(*right, context)?;
                // Could break
                let _ = context.environment.set(var_ref, expr, is_definition)?;

                return Ok(ValueHolder::Void);
            } else if let Expression::Member { object, property } = *left {
                let value = eval_expr(*object, context)?;
                
                if let ValueHolder::Object(ObjectRef { object_id }) = value {
                    let value =  match *property {
                        Expression::Literal { r#type: _, value } => value,
                        _ => {
                            return Err(RuntimeError::InvalidType(
                                "Property should be a variable".to_string(),
                            ));
                        }
                    };

                    let ValueHolder::String(property) = value else {
                        return Err(RuntimeError::InvalidType(
                            "Property should be a variable".to_string(),
                        ));
                    };

                    let object_ref = ObjectRef { object_id };
                    let expr = eval_expr(*right, context)?;
                    object_ref.set(property, expr, context);

                    return Ok(ValueHolder::Void);
                } else {
                    return Err(RuntimeError::InvalidType(
                        "Cannot access property on type other than Object".to_string(),
                    ));
                }
            }

            panic!("Expected variable for assignment got {:?}", *left);
        }
        Expression::Call { callee, arguments } => eval_call(callee, arguments, context),
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
        other => {
            println!("DEBUG: Left operand is not a number: {:?}", other); // Add this line
            return Err(RuntimeError::InvalidType(
                "Expected float value for binary operation".to_string(),
            ));
        }
    };
    let right_val = match eval_expr(*right, context) {
        Ok(ValueHolder::Float(f)) => f,
        Ok(ValueHolder::Int(f)) => f.into(),
        _ => {
            return Err(RuntimeError::InvalidType(
                "Expected float value for binary operation".to_string(),
            ));
        }
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
    callee: Box<Expression>,
    call_arguments: Vec<Expression>,
    context: &mut ProgramContext,
) -> RuntimeResult {
    if let Expression::Literal {
        value: ValueHolder::String(name),
        ..
    } = *callee.clone()
    {
        let func = BUILT_IN_FUNCTIONS.get(&name);

        if let Some(func) = func {
            let arguments: Vec<ValueHolder> = call_arguments
                .into_iter()
                .map(|arg| eval_expr(arg, context))
                .collect::<Result<Vec<_>, RuntimeError>>()?;

            return Ok(func(arguments));
        };
    }

    if let ValueHolder::Fn(Function {
        arguments,
        statements,
        scope_position
    }) = eval_expr(*callee.clone(), context)?
    {
        if arguments.len() != call_arguments.len() {
            return Err(RuntimeError::Custom("Invalid number of args".to_string()));
        }

        // Evaluate all arguments before entering the scope to avoid multiple mutable borrows
        let evaluated_args: Vec<ValueHolder> = call_arguments
            .into_iter()
            .map(|arg| eval_expr(arg, context))
            .collect::<Result<Vec<_>, RuntimeError>>()?;

        context
            .environment
            .enter_scope(ScopeKind::Call(scope_position as i32));
        
        context
            .environment
            .set(VariableRef { slot: 0, depth: 0 }, ValueHolder::Fn(Function { arguments: arguments.clone(), statements: statements.clone(), scope_position }), true)?;

        for (argument, value) in arguments.iter().zip(evaluated_args.iter()) {
            context
                .environment
                .set(argument.clone(), value.clone(), true)?;
        }
        let return_value = eval_body(&statements, context);
        context.environment.exit_scope();
        return return_value;
    }

    panic!("Tried to call invalid function expression: {:?}", *callee);
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
        _ => Err(RuntimeError::Custom(
            "Invalid equality operator".to_string(),
        )),
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

    Ok(ValueHolder::Bool(match operator {
        Token::GT => left > right,
        Token::GTE => left >= right,
        Token::LT => left < right,
        Token::LTE => left <= right,
        _ => unreachable!(),
    }))
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
        _ => Err(RuntimeError::Custom(
            "Invalid relational operator".to_string(),
        )),
    }
}
