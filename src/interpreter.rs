use core::panic;
use std::{cell::{RefCell, RefMut}, collections::HashMap, fmt::{self}, ops::BitOrAssign, rc::Rc, time::{SystemTime, UNIX_EPOCH}};

use indexmap::IndexSet;
use lazy_static::lazy_static;

use crate::{
    errors::{LanguageError, LanguageErrorTrait, LanguageResult}, interpreter::prototypes::{
        ARRAY_PROTOTYPE, FLOAT_PROTOTYPE, INT_PROTOTYPE, OBJECT_PROTOTYPE, Operation, Prototype, STRING_PROTOTYPE
    }, lexer::TokenKind, loader::Module, parser::{
        ArrayRef, Block, BuiltInFunction, Expression, FunctionKind, LiteralExpressionKind, ObjectRef, Program, RuntimeFunction, Statement, ValueHolder, VariableRef
    }, stdlib::FUNCTION_TABLE
};

use inline_colorization::*;

pub mod prototypes;

#[derive(Debug)]
pub enum RuntimeError {
    InvalidType(String),
    Custom(String),
    VariableNotFound(String),
    NoSuchProperty(String),
    OperationNotSupported(String),
}

impl LanguageErrorTrait for RuntimeError {}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidType(value) => write!(f, "[InvalidType] {}", value),
            RuntimeError::Custom(value) => write!(f, "[CustomError] {}", value),
            RuntimeError::VariableNotFound(name) => write!(f, "[VariableNotFound] {}", name),
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
            ValueHolder::Array(_) => &*ARRAY_PROTOTYPE,
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
            ValueHolder::Array(_) => format!("ArrayRef"),
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
            ValueHolder::Array(_) => write!(f, "{}", self.to_string()),
            ValueHolder::LazyRef { .. } => write!(f, "{}", self.to_string()),
            ValueHolder::Void => write!(f, "{}", self.to_string()),
        }
    }
}

pub type RuntimeResult = LanguageResult<ValueHolder>;

type BuiltInFunctionMethod = HashMap<String, Box<dyn Fn(Vec<ValueHolder>, Rc<RefCell<ModuleContext>>) -> RuntimeResult + Send + Sync>>;

#[derive(Debug)]
pub struct ProgramContext {
    pub modules: HashMap<String, Rc<RefCell<Module>>>,
    pub schemas: Vec<Schema>,
    pub store: HashMap<usize, Object>
}

impl ProgramContext {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            schemas: vec![],
            store: HashMap::new()
        }
    }

    pub fn get_module(&self, source: &str) -> Option<RefMut<'_, Module>> {
        self.modules.get(source).map(|value| value.borrow_mut())
    }
}

#[derive(Debug, Clone)]
pub struct ModuleContext {
    pub environment: Environment,
    pub program: Rc<RefCell<ProgramContext>>,
}

impl ModuleContext {
    pub fn new(program: Rc<RefCell<ProgramContext>>) -> Self {
        let environment: Environment = Environment::new();

        Self {
            environment,
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

fn get_schema(keys: IndexSet<String>, context: Rc<RefCell<ModuleContext>>) -> Schema {
    let schema =  context
        .borrow()
        .program
        .borrow()
        .schemas
        .iter()
        .find(|s| s.keys == keys)
        .cloned();
    
    schema.unwrap_or_else(|| {
        let schema: Schema = Schema {
            id: context.borrow().program.borrow().schemas.len(),
            keys,
        };
        context.borrow().program.borrow_mut().schemas.push(schema.clone());
        schema
    })
}

impl ObjectRef {
    pub fn new(entries: HashMap<String, ValueHolder>, context: Rc<RefCell<ModuleContext>>) -> Self {
        let keys: IndexSet<String> = entries.keys().cloned().collect();

        let schema = get_schema(keys, context.clone());

        let object = Object {
            schema_id: schema.id,
            values: entries.values().cloned().collect(),
        };

        let context = context.borrow();
        let program = &mut context.program.borrow_mut();

        let object_id = program.store.len() + 1;

        program.store.insert(object_id, object);

        ObjectRef { object_id }
    }

    pub(self) fn get(self, property: String, context_ref: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.object_id)
            .expect("Object not found");

        let schema = program
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let index = schema.keys.iter().position(|key| *key == property);

        if let Some(index) = index {
            return Ok(object.values[index].clone());
        }

        Err(LanguageError::from(RuntimeError::NoSuchProperty(property)))
    }

    pub(self) fn set(&self, property: String, value: ValueHolder, context_ref: Rc<RefCell<ModuleContext>>) {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.object_id)
            .expect("Object not found");

        let schema = program
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

        drop(program);
        
        let schema = get_schema(keys, context_ref.clone());

        let object = Object {
            schema_id: schema.id,
            values,
        };

        let program = &mut context.program.borrow_mut();

        program.store.insert(self.object_id, object);
    }

    pub fn fetch(&self, context_ref: Rc<RefCell<ModuleContext>>) -> HashMap<String, ValueHolder> {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.object_id)
            .expect("Object not found");

        let schema = program
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let mut map = HashMap::new();

        for i in 0..schema.keys.len() {
            let key = schema.keys[i].clone(); 
            let value = object.values[i].clone();

            map.insert(key, value);
        }

        map
    }
}

impl ArrayRef {
    pub(self) fn new(items: Vec<ValueHolder>, context: Rc<RefCell<ModuleContext>>) -> Self {
        let context = context.borrow();
        let program = &mut context.program.borrow_mut();

        let array_id = program.store.len() + 1;

        // Placeholder implementation
        program.store.insert(array_id, Object { schema_id: 0, values: items });

        ArrayRef { array_id }
    }

    pub fn get(&self, index: usize, context_ref: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.array_id)
            .expect("Array not found");

        let value = object.values.get(index).cloned().ok_or(LanguageError::from(RuntimeError::Custom(format!(
            "Index {} out of bounds",
            index
        ))))?;

        Ok(value)
    }

    pub fn set(&self, index: usize, value: ValueHolder, context_ref: Rc<RefCell<ModuleContext>>) {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.array_id)
            .expect("Array not found");

        let mut values = object.values.clone();

        if index >= values.len() {
            values.push(value);
        } else {
            values[index] = value;
        }

        drop(program);

        let object = Object {
            schema_id: 0,
            values,
        };

        let program = &mut context.program.borrow_mut();

        program.store.insert(self.array_id, object);
    }

    pub fn fetch(&self, context_ref: Rc<RefCell<ModuleContext>>) -> Vec<ValueHolder> {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.array_id)
            .expect("Array not found");

        object.values.clone()
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
    context: Rc<RefCell<ModuleContext>>
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub scopes: Vec<Rc<RefCell<Scope>>>,
    pub context: Option<Rc<RefCell<ModuleContext>>>
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![],
            context: None
        }
    }

    pub fn init(&mut self, context: Rc<RefCell<ModuleContext>>) {
        self.context = Some(context.clone());

        self.scopes.push(Rc::new(RefCell::new(Scope {
            kind: ScopeKind::Program,
            slots: vec![],
            interrupted: None,
            parent: None,
            context: context
        })));
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Rc::new(RefCell::new(Scope {
            kind,
            slots: vec![],
            interrupted: None,
            parent: self.scopes.last().cloned(),
            context: self.context.as_ref().expect("Environment not initilialized").clone()
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
    ) -> LanguageResult<()> {
        if !define {
            // Traverse up the parent chain to find the correct scope
            let mut current_scope = self.scopes.last().unwrap().clone();
            for _ in 0..var_ref.depth {
                let next_scope = {
                    let scope = current_scope.borrow();
                    match &scope.kind {
                        ScopeKind::Call(inner_scope) => inner_scope.clone(),
                        _ => scope.parent.clone().ok_or(LanguageError::from(RuntimeError::VariableNotFound(format!(
                            "Parent scope not found for slot {} at depth {}",
                            var_ref.slot, var_ref.depth
                        ))))?
                    }
                };

                current_scope = next_scope;
            }
            current_scope.borrow_mut()
                .slots
                .get_mut(var_ref.slot)
                .map(|v| *v = value)
                .ok_or(LanguageError::from(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    var_ref.slot, var_ref.depth
                ))))?;
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
    context: Rc<RefCell<ModuleContext>>,
) -> LanguageResult<()> {
    eval_body(&program.body, context)?;

    Ok(())
}

fn define_functions(
    statements: &Vec<Statement>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> LanguageResult<()> {
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

        let mut context = context_ref.borrow_mut();

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

fn eval_body(statements: &Vec<Statement>, context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    define_functions(statements, context.clone())?;

    for statement in statements {
        let value = eval_statement(statement.clone(), context.clone())?;
        if let Some(value) = &context.borrow().environment.scopes.last().unwrap().borrow().interrupted {
            return Ok(value.clone());
        }

        for mut scope in context.borrow().environment.scopes.iter().rev().map(|scope| scope.borrow_mut()) {
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

fn eval_statement(statement: Statement, context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
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
        Statement::ImportIR { .. } => Err(LanguageError::with_source(RuntimeError::Custom(
            "Import declarations can only be at the top of modules".into(),
        ), 0, 0)),
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
    context_ref: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let mut iter: Box<dyn Iterator<Item = i32>> = Box::new(0..);

    match (&left, &right) {
        (Some(_), Some(_)) => {
            let left: i32 = eval_expr(left.unwrap(), context_ref.clone())?.into();
            let right: i32 = eval_expr(right.unwrap(), context_ref.clone())?.into();

            iter = if left < right {
                Box::new(left..right)
            } else {
                Box::new(((right + 1)..(left + 1)).rev())
            };
        }
        (Some(_), None) => {
            let left: i32 = eval_expr(left.unwrap(), context_ref.clone())?.into();

            iter = Box::new(left..)
        }
        (None, Some(_)) => {
            let right: i32 = eval_expr(right.unwrap(), context_ref.clone())?.into();

            iter = Box::new(0..right)
        }
        _ => (),
    }

    context_ref.borrow_mut().environment.enter_scope(ScopeKind::Loop);
    context_ref
        .borrow_mut()
        .environment
        .set(variable.clone(), ValueHolder::Int(0), true)?;

    for i in iter {
        {
            let context = context_ref.borrow();
            // Faster than environment.set in this context
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

            scope.slots.fill(ValueHolder::Void);

            scope.slots[variable.slot] = ValueHolder::Int(i);
        }

        eval_body(&statements, context_ref.clone())?;
        let broken = context_ref
            .borrow()
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
    context_ref.borrow_mut().environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_while(
    condition: Expression,
    statements: Vec<Statement>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    context_ref.borrow_mut().environment.enter_scope(ScopeKind::Loop);

    fn execute_loop_body(
        statements: &Vec<Statement>,
        context_ref: &Rc<RefCell<ModuleContext>>
    ) -> LanguageResult<bool> {
        {
            let context = context_ref.borrow();
            let scope = &mut context.environment.scopes.last().unwrap().borrow_mut();

            scope.slots.fill(ValueHolder::Void);
        }

        eval_body(statements, context_ref.clone())?;
        let broken = context_ref
            .borrow()
            .environment
            .scopes
            .last()
            .unwrap()
            .borrow()
            .interrupted
            .is_some();

        return Ok(broken)
    }

    let optimized_condition = if let Expression::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::Bool(value) } = condition {
        Some(value)
    } else {
        None
    };

    if let Some(value) = optimized_condition {
        if value {
            loop {
                if execute_loop_body(&statements, &context_ref)? {
                    break;
                }
            }
        }
    } else {
        while eval_expr(condition.clone(), context_ref.clone())?.into() {
            if execute_loop_body(&statements, &context_ref)? {
                break;
            }
        }
    }

    context_ref.borrow_mut().environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_if(
    condition: Expression,
    block: Block,
    alternate: Option<Box<Statement>>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let cond: bool = eval_expr(condition, context_ref.clone())?.into();

    context_ref.borrow_mut().environment.enter_scope(ScopeKind::Regular);

    if cond {
        eval_body(&block.statements, context_ref.clone())?;
    } else if let Some(box Statement::If {
        condition,
        block,
        alternate,
    }) = alternate
    {
        eval_if(condition, block, alternate, context_ref.clone())?;
    } else if let Some(box Statement::Block(Block { statements })) = alternate {
        eval_body(&statements, context_ref.clone())?;
    }
    context_ref.borrow_mut().environment.exit_scope();
    Ok(ValueHolder::Void)
}

fn eval_expr(expr: Expression, context_ref: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    return match expr {
        Expression::Binary {
            left,
            operator,
            right,
        } => eval_binary(left, operator, right, context_ref),
        Expression::Member { object, property } => {
            let value = eval_expr(*object, context_ref.clone())?;

            let property = eval_expr(*property, context_ref.clone())?;

            if matches!(&property, ValueHolder::Int(_) | ValueHolder::Float(_)) {
                let index = match &property {
                    ValueHolder::Int(i) => *i as usize,
                    ValueHolder::Float(f) => *f as usize,
                    _ => unreachable!()
                };

                if let ValueHolder::Array(array_ref) = &value {
                    return array_ref.get(index, context_ref.clone());
                } else {
                    return Err(LanguageError::with_source(RuntimeError::InvalidType(
                        "Cannot access index on type other than Array".to_string(),
                    ), 0, 0));
                }
            }

            let ValueHolder::String(property) = property else {
                return Err(LanguageError::with_source(RuntimeError::InvalidType(
                    "Property should be a string".to_string(),
                ), 0, 0));
            };

            if let ValueHolder::Object(object) = &value {
                match object.clone().get(property.clone(), context_ref.clone()) {
                    Ok(object) => return Ok(object),
                    Err(_) => (),
                };
            };

            let prototype = value.get_prototype();

            let method = Rc::new(
                prototype
                    .get_method(&property)
                    .ok_or(LanguageError::with_source(RuntimeError::NoSuchProperty(property), 0, 0))?
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
                    .map(|(k, v)| (k.clone(), eval_expr(v.clone(), context_ref.clone()).unwrap()))
                    .collect();
                let object_ref = ObjectRef::new(entries, context_ref.clone());
                return Ok(ValueHolder::Object(object_ref));
            } else if let LiteralExpressionKind::Array(items) = r#type {
                let items: Vec<ValueHolder> = items
                    .iter()
                    .map(|v| eval_expr(v.clone(), context_ref.clone()).unwrap())
                    .collect();
                // ArrayRef creation to be implemented
                return Ok(ValueHolder::Array(ArrayRef::new(items, context_ref.clone()))); // Placeholder
            } else if let LiteralExpressionKind::Variable = r#type {
                return Err(LanguageError::with_source(RuntimeError::VariableNotFound(value.to_string()), 0, 0));
            } else {
                unreachable!();
            }
        }
        Expression::Variable(variable) => {
            let value = context_ref.borrow().environment.get(variable.clone()).ok_or(
                LanguageError::with_source(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    variable.slot, variable.depth
                )), 0, 0),
            )?;

            match value {
                ValueHolder::LazyRef { slot, module: source } => {
                    let program_ref = context_ref.borrow().program.clone();
                    let program = program_ref.borrow();

                    let is_loaded = program.modules.get(&source).unwrap().borrow().is_loaded();

                    if !is_loaded {
                        let module = program.modules.get(&source).unwrap().clone();
                        drop(program);
                        Module::execute(module)?
                    }

                    let program = program_ref.borrow();

                    let Some(module) = program.modules.get(&source).map(|module| module.borrow()) else {
                        unreachable!()
                    };

                    let context = module.context.clone().unwrap();  
                    let context = context.borrow();

                    let environment= context.environment.scopes.first().unwrap().borrow();

                    let value = environment.slots.get(slot).unwrap();
                    
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
                let expr = eval_expr(*right, context_ref.clone())?;
                // Could break
                context_ref.borrow_mut().environment.set(var_ref, expr, is_definition)?;

                return Ok(ValueHolder::Void);
            } else if let Expression::Member { object, property } = *left {
                let value = eval_expr(*object, context_ref.clone())?;

                if let ValueHolder::Object(ObjectRef { object_id }) = value {
                    let value = match *property {
                        Expression::Literal { r#type: _, value } => value,
                        Expression::Variable(_) => eval_expr(*property, context_ref.clone())?,
                        _ => {
                            return Err(LanguageError::with_source(RuntimeError::InvalidType(
                                "Property should be a variable".to_string(),
                            ), 0, 0));
                        }
                    };

                    let ValueHolder::String(property) = value else {
                        return Err(LanguageError::with_source(RuntimeError::InvalidType(
                            "Property should be a variable".to_string(),
                        ), 0, 0));
                    };

                    let object_ref = ObjectRef { object_id };
                    let expr = eval_expr(*right, context_ref.clone())?;
                    object_ref.set(property, expr, context_ref.clone());

                    return Ok(ValueHolder::Void);
                } else if let ValueHolder::Array(array_ref) = value {
                    let index = match *property {
                        Expression::Literal { r#type: _, value } => value,
                        Expression::Variable(_) => eval_expr(*property, context_ref.clone())?,
                        _ => {
                            return Err(LanguageError::with_source(RuntimeError::InvalidType(
                                "Index should be a variable".to_string(),
                            ), 0, 0));
                        }
                    };

                    let index = match index {
                        ValueHolder::Int(i) => i as usize,
                        ValueHolder::Float(f) => f as usize,
                        _ => {
                            return Err(LanguageError::with_source(RuntimeError::InvalidType(
                                "Index should be an integer".to_string(),
                            ), 0, 0));
                        }
                    };

                    let expr = eval_expr(*right, context_ref.clone())?;
                    array_ref.set(index, expr, context_ref.clone());

                    return Ok(ValueHolder::Void);
                } else {
                    return Err(LanguageError::with_source(RuntimeError::InvalidType(
                        "Cannot access property on type other than Object".to_string(),
                    ), 0, 0));
                }
            }

            panic!("Expected variable for assignment got {:?}", *left);
        }
        Expression::Call { callee, arguments } => eval_call(callee, arguments, context_ref),
        Expression::Equality {
            left,
            operator,
            right,
        } => eval_equality(left, operator, right, context_ref),
        Expression::Relational {
            left,
            operator,
            right,
        } => eval_relational(left, operator, right, context_ref),
        Expression::Logical {
            left,
            operator,
            right,
        } => eval_logical(left, operator, right, context_ref),
    };
}

fn eval_binary(
    left: Box<Expression>,
    operator: TokenKind,
    right: Box<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = &eval_expr(*left, context.clone())?;
    let right = &eval_expr(*right, context)?;
    let left_proto = left.get_prototype();

    match operator {
        TokenKind::Plus => left_proto.operate(Operation::Addition, left, right),
        TokenKind::Minus => left_proto.operate(Operation::Substraction, left, right),
        TokenKind::Asterisk => left_proto.operate(Operation::Multiplication, left, right),
        TokenKind::Slash => left_proto.operate(Operation::Division, left, right),
        TokenKind::Percent => left_proto.operate(Operation::Modulo, left, right),
        _ => panic!("Invalid binary operator"),
    }
}

fn eval_call(
    callee: Box<Expression>,
    call_arguments: Vec<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let evaluated_args: Vec<ValueHolder> = call_arguments
        .into_iter()
        .map(|arg| eval_expr(arg, context.clone()))
        .collect::<LanguageResult<Vec<_>>>()?;
    
    if let Expression::Literal {
        value: ValueHolder::String(name),
        ..
    } = *callee.clone()
    {
        let table = FUNCTION_TABLE.lock().unwrap();
        let func = table.get(name.as_str()).cloned();

        drop(table);

        if let Some(func) = func {
            return func(&evaluated_args, context.clone());
        };
    }

    let expr = eval_expr(*callee.clone(), context.clone())?;

    if let ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
        arguments,
        statements,
        scope
    })) = expr
    {
        if arguments.len() != evaluated_args.len() {
            return Err(LanguageError::with_source(RuntimeError::Custom("Invalid number of args".to_string()), 0 ,0));
        }

        let context = scope.borrow().context.clone();

        context
            .borrow_mut()
            .environment
            .enter_scope(ScopeKind::Call(scope.clone()));

        context.borrow_mut().environment.set(
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
                .borrow_mut()
                .environment
                .set(argument.clone(), value.clone(), true)?;
        }

        let return_value = eval_body(&statements, context.clone());
        context.borrow_mut().environment.exit_scope();
        return return_value;
    }

    if let ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction { func, instance })) = expr {
        return func(&instance, evaluated_args, context.clone());
    }

    panic!("Tried to call invalid function expression: {:?}", *callee);
}

fn eval_equality(
    left: Box<Expression>,
    operator: TokenKind,
    right: Box<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(*left, context.clone())?;
    let right = eval_expr(*right, context)?;

    match operator {
        TokenKind::EQ => Ok(ValueHolder::Bool(left == right)),
        TokenKind::NE => Ok(ValueHolder::Bool(left != right)),
        _ => Err(LanguageError::with_source(RuntimeError::Custom(
            "Invalid equality operator".to_string(),
        ), 0 ,0)),
    }
}

fn eval_relational(
    left: Box<Expression>,
    operator: TokenKind,
    right: Box<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(*left, context.clone())?;
    let right = eval_expr(*right, context)?;

    Ok(ValueHolder::Bool(match operator {
        TokenKind::GT => left > right,
        TokenKind::GTE => left >= right,
        TokenKind::LT => left < right,
        TokenKind::LTE => left <= right,
        _ => unreachable!(),
    }))
}

fn eval_logical(
    left: Box<Expression>,
    operator: TokenKind,
    right: Box<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(*left, context.clone())?;
    let right = eval_expr(*right, context)?;

    match operator {
        TokenKind::And => Ok(ValueHolder::Bool(left.into() && right.into())),
        TokenKind::Or => Ok(ValueHolder::Bool(left.into() || right.into())),
        _ => Err(LanguageError::with_source(RuntimeError::Custom(
            "Invalid relational operator".to_string(),
        ), 0 ,0)),
    }
}

// fn eval_imports(
//     statements: &Vec<Statement>,
//     context: Rc<RefCell<ModuleContext>>,
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
