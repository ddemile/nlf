use core::panic;
use std::{
    cell::{RefCell, RefMut}, collections::HashMap, fmt::{self}, rc::Rc, time::{SystemTime, UNIX_EPOCH}
};

use indexmap::IndexSet;
use lazy_static::lazy_static;

use crate::{
    interpreter::prototypes::{
        FLOAT_PROTOTYPE, INT_PROTOTYPE, OBJECT_PROTOTYPE, Operation, Prototype, STRING_PROTOTYPE,
    },
    lexer::{Token},
    loader::Module,
    parser::{
        Block, BuiltInFunction, Expression, FunctionKind, LiteralExpressionKind, ObjectRef,
        Program, RuntimeFunction, Statement, ValueHolder, VariableRef,
    }
};

use inline_colorization::*;

pub mod prototypes;

#[derive(Debug)]
pub enum RuntimeError {
    InvalidType(String),
    Custom(String),
    VariableNotFound(String),
    VariableAlreadyDeclared(String),
    NoSuchProperty(String),
    OperationNotSupported(String),
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
            RuntimeError::OperationNotSupported(name) => {
                write!(f, "[OperationNotSupported] {}", name)
            }
        }
    }
}

impl ValueHolder {
    fn get_prototype(&self) -> &Prototype {
        match self {
            ValueHolder::String(_) => &*STRING_PROTOTYPE,
            ValueHolder::Int(_) => &*INT_PROTOTYPE,
            ValueHolder::Float(_) => &*FLOAT_PROTOTYPE,
            ValueHolder::Object(_) => &*OBJECT_PROTOTYPE,
            _ => todo!(),
        }
    }

    fn to_string(&self) -> String {
        match self {
            ValueHolder::String(value) => format!("{value}"),
            ValueHolder::Float(value) => format!("{value}"),
            ValueHolder::Int(value) => format!("{value}"),
            ValueHolder::Bool(value) => format!("{value}"),
            ValueHolder::Fn(FunctionKind::Runtime(_)) => format!("fn() {{ TODO }}"),
            ValueHolder::Fn(FunctionKind::BuiltIn(_)) => format!("fn() {{ native code }}"),
            ValueHolder::Object(_) => format!("ObjectRef"),
            ValueHolder::LazyRef { .. } => format!("LazyRef"),
            ValueHolder::Void => format!("Void"),
        }
    }
}

impl fmt::Display for ValueHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueHolder::String(_) => write!(f, "{}", self.to_string()),
            ValueHolder::Float(_) => write!(f, "{color_yellow}{}{color_reset}", self.to_string()),
            ValueHolder::Int(_) => write!(f, "{color_yellow}{}{color_reset}", self.to_string()),
            ValueHolder::Bool(_) => write!(f, "{color_blue}{}{color_reset}", self.to_string()),
            ValueHolder::Fn(_) => write!(f, "{color_black}{}{color_reset}", self.to_string()),
            ValueHolder::Object(_) => write!(f, "{}", self.to_string()),
            ValueHolder::LazyRef { .. } => write!(f, "{}", self.to_string()),
            ValueHolder::Void => write!(f, "{}", self.to_string()),
        }
    }
}

pub type RuntimeResult = Result<ValueHolder, RuntimeError>;

lazy_static! {
    static ref BUILT_IN_FUNCTIONS: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> RuntimeResult + Send + Sync>> = {
        let mut m: HashMap<String, Box<dyn Fn(Vec<ValueHolder>) -> RuntimeResult + Send + Sync>> =
            HashMap::new();

        m.insert(
            String::from("print"),
            Box::new(|arguments| {
                let arguments: Vec<String> = arguments
                    .iter()
                    .map(|argument| -> String { format!("{argument}") })
                    .collect();

                println!("{}", arguments.join(" "));

                Ok(ValueHolder::Void)
            }),
        );

        m.insert(
            String::from("pow"),
            Box::new(|arguments| {
                let a = match arguments.get(0).unwrap() {
                    ValueHolder::Float(f) => Ok(f),
                    _ => Err(RuntimeError::InvalidType(
                        "Expected float value".to_string(),
                    )),
                }?;
                let b = match arguments.get(1).unwrap() {
                    ValueHolder::Float(f) => Ok(f),
                    _ => Err(RuntimeError::InvalidType(
                        "Expected float value".to_string(),
                    )),
                }?;

                Ok(ValueHolder::Float(a.powf(*b)))
            }),
        );

        m.insert(
            String::from("now"),
            Box::new(|_| {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("time should go forward");
                Ok(ValueHolder::Float(since_the_epoch.as_millis_f64()))
            }),
        );

        m.insert(
            String::from("assert"),
            Box::new(|arguments| {
                let a = match arguments.get(0).unwrap() {
                    ValueHolder::Bool(f) => f,
                    _ => panic!("Expected bool value"),
                };

                if !a {
                    return Err(RuntimeError::Custom("Assertion failed".to_string()));
                }

                Ok(ValueHolder::Void)
            }),
        );

        m
    };
}

#[derive(Debug, Clone)]
pub struct ProgramContext {
    pub modules: HashMap<String, RefCell<Module>>,
}

impl ProgramContext {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new()
        }
    }

    pub fn get_module(&self, source: &str) -> Option<RefMut<'_, Module>> {
        self.modules.get(source).map(|value| value.borrow_mut())
    }
}

#[derive(Debug, Clone)]
pub struct ModuleContext {
    pub environment: Environment,
    pub schemas: Vec<Schema>,
    pub store: HashMap<usize, Object>,
    pub program: Rc<RefCell<ProgramContext>>,
}

impl ModuleContext {
    pub fn new(program: Rc<RefCell<ProgramContext>>) -> Self {
        let environment: Environment = Environment::new();

        Self {
            environment,
            schemas: vec![],
            store: HashMap::new(),
            program,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Schema {
    id: usize,
    keys: IndexSet<String>,
}

#[derive(Debug, Clone)]
pub struct Object {
    schema_id: usize,
    values: Vec<ValueHolder>,
}

fn get_schema(keys: IndexSet<String>, context: &mut ModuleContext) -> Schema {
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
    pub(self) fn new(entries: HashMap<String, ValueHolder>, context: &mut ModuleContext) -> Self {
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

    pub(self) fn get(self, property: String, context: &ModuleContext) -> RuntimeResult {
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

    pub(self) fn set(&self, property: String, value: ValueHolder, context: &mut ModuleContext) {
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
pub enum ScopeKind {
    Program,
    Call(Rc<RefCell<Scope>>),
    Loop,
    Regular,
}

#[derive(Debug, Clone)]
pub struct Scope {
    kind: ScopeKind,
    slots: Vec<ValueHolder>,
    interrupted: Option<ValueHolder>,
    parent: Option<Rc<RefCell<Scope>>>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub scopes: Vec<Rc<RefCell<Scope>>>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Rc::new(RefCell::new(Scope {
                kind: ScopeKind::Program,
                slots: vec![],
                interrupted: None,
                parent: None,
            }))],
        }
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Rc::new(RefCell::new(Scope {
            kind,
            slots: vec![],
            interrupted: None,
            parent: self.scopes.last().cloned(),
        })));
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
            let mut current_scope = self.scopes.last().unwrap().clone();
            for _ in 0..var_ref.depth {
                let next_scope = {
                    let scope = current_scope.borrow();
                    match &scope.kind {
                        ScopeKind::Call(inner_scope) => inner_scope.clone(),
                        _ => scope.parent.clone().ok_or(RuntimeError::VariableNotFound(format!(
                            "Parent scope not found for slot {} at depth {}",
                            var_ref.slot, var_ref.depth
                        )))?
                    }
                };

                current_scope = next_scope;
            }
            current_scope.borrow_mut()
                .slots
                .get_mut(var_ref.slot)
                .map(|v| *v = value)
                .ok_or(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    var_ref.slot, var_ref.depth
                )))?;
            return Ok(());
        } else if let Some(mut scope) = self.scopes.last_mut().map(|scope| scope.borrow_mut()) {
            if scope.slots.len() <= var_ref.slot {
                scope.slots.resize(var_ref.slot + 1, ValueHolder::Void);
            }
            scope.slots[var_ref.slot] = value;
            return Ok(());
        }
        unreachable!()
    }

    pub fn get(&self, var_ref: VariableRef) -> Option<ValueHolder> {
        // Traverse up the parent chain to find the correct scope
        let mut current_scope = self.scopes.last().unwrap().clone();
        for _ in 0..var_ref.depth {
            let next_scope = {
                let scope = current_scope.borrow();
                match &scope.kind {
                    ScopeKind::Call(inner_scope) => inner_scope.clone(),
                    _ => scope.parent.clone()?
                }
            };

            current_scope = next_scope;
        }
        current_scope.borrow().slots.get(var_ref.slot).cloned()
    }
}

pub fn interpret(
    program: Program,
    context: &mut ModuleContext,
) -> Result<(), RuntimeError> {
    eval_body(&program.body, context)?;

    Ok(())
}

fn define_functions(
    statements: &Vec<Statement>,
    context: &mut ModuleContext,
) -> Result<(), RuntimeError> {
    for statement in statements
        .iter()
        .filter(|statement| matches!(*statement, Statement::FunctionIR { .. } | Statement::Export { declaration: box Statement::FunctionIR { .. } }))
    {
        let Statement::FunctionIR {
            var_ref,
            arguments,
            statements,
        } = statement else {
            panic!("Expected function declaration")
        };

        let scope = context.environment.scopes.last_mut().cloned().unwrap();
        
        context.environment.set(
            var_ref.clone(),
            ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                arguments: arguments.to_vec(),
                statements: statements.to_vec(),
                scope
            })),
            true,
        )?
    }

    Ok(())
}

fn eval_body(statements: &Vec<Statement>, context: &mut ModuleContext) -> RuntimeResult {
    define_functions(statements, context)?;

    for statement in statements {
        let value = eval_statement(statement.clone(), context)?;
        if let Some(value) = &context.environment.scopes.last().unwrap().borrow().interrupted {
            return Ok(value.clone());
        }

        for mut scope in context.environment.scopes.iter().rev().map(|scope| scope.borrow_mut()) {
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

fn eval_statement(statement: Statement, context: &mut ModuleContext) -> RuntimeResult {
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
        Statement::While {
            condition,
            statements,
        } => eval_while(condition, statements, context),
        Statement::FunctionIR {
            var_ref,
            arguments,
            statements,
        } => eval_function(var_ref, arguments, statements),
        Statement::Return { expression } => eval_expr(expression, context),
        Statement::Break => Ok(ValueHolder::Void),
        Statement::Block(_) => Ok(ValueHolder::Void),
        Statement::ImportIR { .. } => Err(RuntimeError::Custom(
            "Import declarations can only be at the top of modules".into(),
        )),
        Statement::Export { declaration } => eval_statement(*declaration, context),
        _ => panic!("Invalid statement : {:?}", statement),
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
    context: &mut ModuleContext,
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
        {
            // Faster than environment.set in this context
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

            for v in &mut scope.slots {
                *v = ValueHolder::Void;
            }

            scope.slots[variable.slot] = ValueHolder::Int(i);
        }

        eval_body(&statements, context)?;
        let broken = context
            .environment
            .scopes
            .last()
            .unwrap()
            .borrow()
            .interrupted
            .is_some();

        if broken {
            break;
        }
    }
    context.environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_while(
    condition: Expression,
    statements: Vec<Statement>,
    context: &mut ModuleContext,
) -> RuntimeResult {
    while eval_expr(condition.clone(), context)?.into() {
        context.environment.enter_scope(ScopeKind::Loop);
        {
            let scope = &mut context.environment.scopes.last_mut().unwrap().borrow_mut();

            for v in &mut scope.slots {
                *v = ValueHolder::Void;
            }
        }

        eval_body(&statements, context)?;
        let broken = context
            .environment
            .scopes
            .last()
            .unwrap()
            .borrow()
            .interrupted
            .is_some();

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
    context: &mut ModuleContext,
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

fn eval_expr(expr: Expression, context: &mut ModuleContext) -> RuntimeResult {
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

            if let ValueHolder::Object(object) = &value {
                match object.clone().get(property.clone(), context) {
                    Ok(object) => return Ok(object),
                    Err(_) => (),
                };
            };

            let prototype = value.get_prototype();

            let method = Rc::new(
                prototype
                    .get_method(&property)
                    .ok_or(RuntimeError::NoSuchProperty(property))?
                    .clone(),
            );

            Ok(ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction {
                func: method,
                instance: Rc::new(value),
            })))
        }
        Expression::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                Ok(value)
            } else if let LiteralExpressionKind::Object(entries) = r#type {
                let entries: HashMap<String, ValueHolder> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), eval_expr(v.clone(), context).unwrap()))
                    .collect();
                let object_ref = ObjectRef::new(entries, context);
                return Ok(ValueHolder::Object(object_ref));
            } else if let LiteralExpressionKind::Variable = r#type {
                return Err(RuntimeError::VariableNotFound(value.to_string()));
            } else {
                unreachable!();
            }
        }
        Expression::Variable(variable) => {
            let value = context.environment.get(variable.clone()).ok_or(
                RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    variable.slot, variable.depth
                )),
            )?;

            match value {
                ValueHolder::LazyRef { slot, module } => {
                    // TODO: check lazy ref and load module if not initialized
                    let program = context.program.borrow();
                    let Some(mut module) = program.get_module(&module) else {
                        unreachable!()
                    };

                    if !module.is_loaded() {
                        module.execute()?
                    }
                                                
                    let context = module.context.borrow();
                    let context = context.as_ref().unwrap();

                    let value = context.environment.get(VariableRef { name: None, slot, depth: 0 }).unwrap();
                    
                    Ok(value.clone())
                }
                value => Ok(value),
            }
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
                    let value = match *property {
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
    context: &mut ModuleContext,
) -> RuntimeResult {
    let left = &eval_expr(*left, context)?;
    let right = &eval_expr(*right, context)?;
    let left_proto = left.get_prototype();

    match operator {
        Token::Plus => left_proto.operate(Operation::Addition, left, right),
        Token::Minus => left_proto.operate(Operation::Substraction, left, right),
        Token::Asterisk => left_proto.operate(Operation::Multiplication, left, right),
        Token::Slash => left_proto.operate(Operation::Division, left, right),
        _ => panic!("Invalid binary operator"),
    }
}

fn eval_call(
    callee: Box<Expression>,
    call_arguments: Vec<Expression>,
    context: &mut ModuleContext,
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

            return func(arguments);
        };
    }

    let expr = eval_expr(*callee.clone(), context)?;

    if let ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
        arguments,
        statements,
        scope
    })) = expr
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
            .enter_scope(ScopeKind::Call(scope.clone()));

        context.environment.set(
            VariableRef {
                name: None,
                slot: 0,
                depth: 0,
            },
            ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                arguments: arguments.clone(),
                statements: statements.clone(),
                scope,
            })),
            true,
        )?;

        for (argument, value) in arguments.iter().zip(evaluated_args.iter()) {
            context
                .environment
                .set(argument.clone(), value.clone(), true)?;
        }
        let return_value = eval_body(&statements, context);
        context.environment.exit_scope();
        return return_value;
    }

    if let ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction { func, instance })) = expr {
        return func(&instance, call_arguments);
    }

    panic!("Tried to call invalid function expression: {:?}", *callee);
}

fn eval_equality(
    left: Box<Expression>,
    operator: Token,
    right: Box<Expression>,
    context: &mut ModuleContext,
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
    context: &mut ModuleContext,
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
    context: &mut ModuleContext,
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

// fn eval_imports(
//     statements: &Vec<Statement>,
//     context: &mut ModuleContext,
// ) -> Result<Vec<Statement>, RuntimeError> {
//     let mut imports_count = 0;

//     for statement in statements {
//         let Statement::ImportIR { specifiers, source } = statement else {
//             break;
//         };

//         if !context.pg_context.borrow().modules.contains_key(source) {
//             let mut module = Module::new(&source);

//             let contents = fs::read_to_string(source)
//                 .map_err(|_| RuntimeError::Custom("Module not found".into()))?;

//             let tokens = lexer::lex(contents);

//             let ast = parser::parse(tokens);

//             let ir = translator::translate(ast);

//             for statement in ir.body {
//                 let Statement::Export {
//                     declaration: box Statement::FunctionIR { var_ref, .. },
//                 } = &statement
//                 else {
//                     continue;
//                 };

//                 module.exports.insert(
//                     var_ref.name.clone().unwrap(),
//                     ValueHolder::LazyRef {
//                         slot: var_ref.slot,
//                         module: source.into(),
//                     },
//                 );
//             }

//             context
//                 .pg_context
//                 .borrow_mut()
//                 .modules
//                 .insert(source.into(), RefCell::new(module));
//         }

//         let program_context = context.pg_context.borrow();
//         let module = program_context.modules.get(source).unwrap();

//         imports_count += 1;

//         for specifier in specifiers {
//             println!("{:?}", specifier);
//             let name = specifier.clone().name.unwrap();
//             let value = module
//                 .exports
//                 .get(&name)
//                 .ok_or(RuntimeError::VariableNotFound(format!(
//                     "Module has no such property: {}",
//                     name
//                 )))?;

//             context
//                 .environment
//                 .set(specifier.clone(), value.clone(), true)?;
//         }
//     }

//     Ok(statements[imports_count..].to_vec())
// }
