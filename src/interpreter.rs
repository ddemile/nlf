use core::panic;
use std::{cell::RefCell, collections::HashMap, fmt::{self}, rc::Rc, sync::{Arc}};

use indexmap::IndexSet;
use parking_lot::{Mutex, MutexGuard};
use serde::Serialize;

use crate::{
    errors::{LanguageError, LanguageErrorTrait, LanguageResult}, interpreter::prototypes::{
        ARRAY_PROTOTYPE, LocalPrototype, NUMBER_PROTOTYPE, OBJECT_PROTOTYPE, Operation, Prototype, STRING_PROTOTYPE
    }, lexer::TokenKind, loader::Module, parser::{
        ArrayRef, Block, BuiltInFunction, Expression, FunctionKind, LiteralExpressionKind, ObjectRef, Program, RuntimeFunction, Statement, ValueHolder, VariableRef, Visibility
    }, stdlib::FUNCTION_TABLE, types::{DynamicNumber, NumberHolder}
};

use inline_colorization::*;

pub mod prototypes;

#[derive(Debug, Serialize, Clone)]
pub struct Field {
    visibility: Visibility,
    name: String,
    value: ValueHolder
}

#[derive(Debug, Serialize, Clone)]
pub struct ClassDefinition {
    fields: Vec<Field>,
    #[serde(skip)]
    prototype: LocalPrototype
}

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
    fn get_prototype(&self) -> &dyn Prototype {
        match self {
            ValueHolder::String(_) => &*STRING_PROTOTYPE,
            ValueHolder::Number(_) => &*NUMBER_PROTOTYPE,
            ValueHolder::Object(_) => &*OBJECT_PROTOTYPE,
            ValueHolder::Array(_) => &*ARRAY_PROTOTYPE,
            _ => todo!(),
        }
    }

    fn to_string(&self) -> String {
        match self {
            ValueHolder::String(value) => format!("{value}"),
            ValueHolder::Number(value) => format!("{value}"),
            ValueHolder::Bool(value) => format!("{value}"),
            ValueHolder::Fn(FunctionKind::Runtime(_)) => format!("fn() {{ TODO }}"),
            ValueHolder::Fn(FunctionKind::BuiltIn(_)) => format!("fn() {{ native code }}"),
            ValueHolder::Object(_) => format!("ObjectRef"),
            ValueHolder::Array(_) => format!("ArrayRef"),
            ValueHolder::LazyRef { .. } => format!("LazyRef"),
            ValueHolder::ClassDefinition(_) => format!("ClassDefinition"),
            ValueHolder::Void => format!("Void"),
        }
    }
}

impl fmt::Display for ValueHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueHolder::String(_) => write!(f, "{}", self.to_string()),
            ValueHolder::Number(_) => write!(f, "{color_yellow}{}{color_reset}", self.to_string()),
            ValueHolder::Bool(_) => write!(f, "{color_blue}{}{color_reset}", self.to_string()),
            ValueHolder::Fn(_) => write!(f, "{color_black}{}{color_reset}", self.to_string()),
            ValueHolder::Object(_) => write!(f, "{}", self.to_string()),
            ValueHolder::Array(_) => write!(f, "{}", self.to_string()),
            ValueHolder::LazyRef { .. } => write!(f, "{}", self.to_string()),
            ValueHolder::ClassDefinition(_) => write!(f, "{}", self.to_string()),
            ValueHolder::Void => write!(f, "{}", self.to_string()),
        }
    }
}

pub type RuntimeResult = LanguageResult<ValueHolder>;

#[derive(Debug)]
pub struct ProgramContext {
    pub modules: HashMap<String, Arc<Mutex<Module>>>,
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

    pub fn get_module(&self, source: &str) -> Option<MutexGuard<'_, Module>> {
        self.modules.get(source).map(|value| value.lock())
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

#[derive(Clone)]
pub struct Object {
    schema_id: usize,
    values: Vec<ValueHolder>,
    prototype: Option<Rc<dyn Prototype>>
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .finish()
    }
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
        context.borrow_mut().program.borrow_mut().schemas.push(schema.clone());
        schema
    })
}

impl ObjectRef {
    pub fn new(entries: HashMap<String, ValueHolder>, prototype: Option<Rc<dyn Prototype>>, context: Rc<RefCell<ModuleContext>>) -> Self {
        let keys: IndexSet<String> = entries.keys().cloned().collect();

        let schema = get_schema(keys, context.clone());

        let object = Object {
            schema_id: schema.id,
            values: entries.values().cloned().collect(),
            prototype
        };

        let context = context.borrow();
        let mut program = context.program.borrow_mut();

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

        let prototype = object.prototype.clone();

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
        drop(context);
        
        let schema = get_schema(keys, context_ref.clone());

        let object = Object {
            schema_id: schema.id,
            values,
            prototype
        };

        let context = context_ref.borrow();
        let mut program = context.program.borrow_mut();

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

    pub fn get_prototype(&self, context_ref: Rc<RefCell<ModuleContext>>) -> Option<Rc<dyn Prototype>> {
        let context = context_ref.borrow();
        let program = context.program.borrow();

        let object = program
            .store
            .get(&self.object_id)
            .expect("Object not found");

        object.prototype.clone()
    }
}

impl ArrayRef {
    pub(self) fn new(items: Vec<ValueHolder>, context: Rc<RefCell<ModuleContext>>) -> Self {
        let context = context.borrow();
        let mut program = context.program.borrow_mut();

        let array_id = program.store.len() + 1;

        // Placeholder implementation
        program.store.insert(array_id, Object { schema_id: 0, values: items, prototype: None });

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
            prototype: None
        };

        let mut program = context.program.borrow_mut();

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

    pub fn push(&self, value: ValueHolder, context_ref: Rc<RefCell<ModuleContext>>) {
        let context = context_ref.borrow();
        let mut program = context.program.borrow_mut();

        let object = program
            .store
            .get_mut(&self.array_id)
            .expect("Array not found");

        object.values.push(value);
    }

    pub fn reverse(&self, context_ref: Rc<RefCell<ModuleContext>>) {
        let context = context_ref.borrow();
        let mut program = context.program.borrow_mut();

        let object = program
            .store
            .get_mut(&self.array_id)
            .expect("Array not found");

        object.values.reverse();
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
        var_ref: &VariableRef,
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

    pub fn get(&self, var_ref: &VariableRef) -> Option<ValueHolder> {
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

fn hoist_declarations(
    statements: &Vec<Statement>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> LanguageResult<()> {
    for statement in statements
        .iter()
        .filter(|statement| matches!(*statement, Statement::FunctionIR { .. } | Statement::ClassIR { .. }))
    {
        match statement {
            Statement::FunctionIR { var_ref, arguments, statements } => {
                let mut context = context_ref.borrow_mut();

                let scope = context.environment.scopes.last_mut().cloned().unwrap();
                
                context.environment.set(
                    var_ref,
                    ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                        arguments: arguments.to_vec(),
                        statements: statements.to_vec(),
                        scope
                    })),
                    true,
                )?
            }
            Statement::ClassIR { var_ref, methods: raw_methods, fields: raw_fields } => {
                let mut context = context_ref.borrow_mut();

                let mut fields: Vec<Field> = vec![];

                let mut prototype = LocalPrototype::new(var_ref.name.clone().unwrap().as_ref());

                prototype.with_operator(Operation::Addition, Box::new(move |_a: &ValueHolder, _b: &ValueHolder| {
                    Ok(ValueHolder::Number(DynamicNumber::from_str("545")))
                }));

                for method in raw_methods {
                    let Statement::Method { name, arguments, statements } = method.clone() else {
                        unreachable!()
                    };

                    let arguments = arguments.clone();
                    let statements = statements.clone();
                    
                    prototype.with_method(&name, Box::new(move |this: &ValueHolder, call_arguments: Vec<ValueHolder>, context_ref: Rc<RefCell<ModuleContext>>| {
                        let scope = context_ref.borrow_mut().environment.scopes.last_mut().unwrap().clone();
  
                        if arguments.len() != call_arguments.len() {
                            return Err(LanguageError::with_source(RuntimeError::Custom("Invalid number of args".to_string()), 0 ,0));
                        }

                        let context = scope.borrow().context.clone();

                        context
                            .borrow_mut()
                            .environment
                            .enter_scope(ScopeKind::Call(scope.clone()));

                        context.borrow_mut().environment.set(
                            &VariableRef {
                                name: None,
                                slot: 0,
                                depth: 0,
                            },
                            ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                                arguments: arguments.clone(),
                                statements: statements.clone(),
                                scope: scope.clone(),
                            })),
                            true,
                        )?;

                        if let ValueHolder::Object(object_ref) = this {
                            context.borrow_mut().environment.set(
                                &VariableRef {
                                    name: None,
                                    slot: 1,
                                    depth: 0,
                                },
                                ValueHolder::Object(*object_ref),
                                true,
                            )?
                        } else {
                            unreachable!()
                        };

                        for (argument, value) in arguments.iter().zip(call_arguments.iter()) {
                            context
                                .borrow_mut()
                                .environment
                                .set(argument, value.clone(), true)?;
                        }

                        let return_value = eval_body(&statements, context.clone());
                        context.borrow_mut().environment.exit_scope();
                        return_value
                    }));
                }

                for field in raw_fields {
                    let Statement::Field { visibility, name, value } = field.clone() else {
                        unreachable!()
                    };

                    fields.push(Field {
                        visibility: visibility,
                        name: name,
                        value: eval_expr(&value, context_ref.clone())?
                    });
                }
                
                context.environment.set(
                    var_ref,
                    ValueHolder::ClassDefinition(ClassDefinition {
                        fields,
                        prototype
                    }),
                    true,
                )?
            }
            _=> ()
        }
    }

    Ok(())
}

fn eval_body(statements: &Vec<Statement>, context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    hoist_declarations(statements, context.clone())?;

    for statement in statements {
        let value = eval_statement(statement, context.clone())?;
        if let Some(value) = &context.borrow().environment.scopes.last().unwrap().borrow().interrupted {
            return Ok(value.clone());
        }

        for mut scope in context.borrow().environment.scopes.iter().rev().map(|scope| scope.borrow_mut()) {
            match (statement, &scope.kind) {
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

fn eval_statement(statement: &Statement, context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    match statement {
        Statement::Expression { expression } => eval_expr(&expression, context),
        Statement::If {
            condition,
            block,
            alternate,
        } => eval_if(&condition, block, alternate, context),
        Statement::ForIR {
            variable,
            left,
            right,
            statements,
        } => eval_for(variable, left, right, statements, context),
        Statement::While {
            condition,
            statements,
        } => eval_while(&condition, statements, context),
        Statement::FunctionIR {
            var_ref,
            arguments,
            statements,
        } => eval_function(var_ref, arguments, statements),
        Statement::Return { expression } => eval_expr(&expression, context),
        Statement::Break => Ok(ValueHolder::Void),
        Statement::Block(_) => Ok(ValueHolder::Void),
        Statement::ImportIR { .. } => Err(LanguageError::with_source(RuntimeError::Custom(
            "Import declarations can only be at the top of modules".into(),
        ), 0, 0)),
        Statement::Export { declaration } => eval_statement(declaration, context),
        Statement::ClassIR { .. } => Ok(ValueHolder::Void),
        _ => panic!("Invalid statement : {:?}", statement),
    }
}

fn eval_function(
    _var_ref: &VariableRef,
    _arguments: &Vec<VariableRef>,
    _statements: &Vec<Statement>,
) -> RuntimeResult {
    Ok(ValueHolder::Void)
}

fn eval_for(
    variable: &VariableRef,
    left: &Option<Expression>,
    right: &Option<Expression>,
    statements: &Vec<Statement>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let mut iter: Box<dyn Iterator<Item = i32>> = Box::new(0..);

    match (&left, &right) {
        (Some(_), Some(_)) => {
            let left: i32 = eval_expr(left.as_ref().unwrap(), context_ref.clone())?.into();
            let right: i32 = eval_expr(right.as_ref().unwrap(), context_ref.clone())?.into();

            iter = if left < right {
                Box::new(left..right)
            } else {
                Box::new(((right + 1)..(left + 1)).rev())
            };
        }
        (Some(_), None) => {
            let left: i32 = eval_expr(left.as_ref().unwrap(), context_ref.clone())?.into();

            iter = Box::new(left..)
        }
        (None, Some(_)) => {
            let right: i32 = eval_expr(right.as_ref().unwrap(), context_ref.clone())?.into();

            iter = Box::new(0..right)
        }
        _ => (),
    }

    context_ref.borrow_mut().environment.enter_scope(ScopeKind::Loop);
    context_ref
        .borrow_mut()
        .environment
        .set(variable, ValueHolder::Number(DynamicNumber::new(NumberHolder::Integer8(0))), true)?;

    for i in iter {
        {
            let context = context_ref.borrow();
            // Faster than environment.set in this context
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

            scope.slots.fill(ValueHolder::Void);

            scope.slots[variable.slot] = ValueHolder::Number(DynamicNumber::new(NumberHolder::Integer32(i)));
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
    condition: &Expression,
    statements: &Vec<Statement>,
    context_ref: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    context_ref.borrow_mut().environment.enter_scope(ScopeKind::Loop);

    fn execute_loop_body(
        statements: &Vec<Statement>,
        context_ref: &Rc<RefCell<ModuleContext>>
    ) -> LanguageResult<bool> {
        {
            let context = context_ref.borrow();
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

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

    let optimized_condition = if let Expression::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::Bool(value) } = *condition {
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
        while eval_expr(condition, context_ref.clone())?.into() {
            if execute_loop_body(&statements, &context_ref)? {
                break;
            }
        }
    }

    context_ref.borrow_mut().environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_if(
    condition: &Expression,
    block: &Block,
    alternate: &Option<Box<Statement>>,
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
        eval_if(&condition, block, alternate, context_ref.clone())?;
    } else if let Some(box Statement::Block(Block { statements })) = alternate {
        eval_body(&statements, context_ref.clone())?;
    }
    context_ref.borrow_mut().environment.exit_scope();
    Ok(ValueHolder::Void)
}

fn eval_expr(expr: &Expression, context_ref: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    return match expr {
        Expression::Binary {
            left,
            operator,
            right,
        } => eval_binary(&left, operator, &right, context_ref),
        Expression::Member { object, property } => {
            let value = eval_expr(object, context_ref.clone())?;

            let property = eval_expr(property, context_ref.clone())?;

            if matches!(&property, ValueHolder::Number(_)) {
                let index = match &property {
                    ValueHolder::Number(f) => <DynamicNumber as Into<usize>>::into(*f),
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
                match object.clone().get(property.to_string(), context_ref.clone()) {
                    Ok(object) => return Ok(object),
                    Err(_) => (),
                };
            };

            let mut prototype: Option<&dyn Prototype> = if let ValueHolder::Object(object) = value {
                if let Some(prototype) = object.get_prototype(context_ref.clone()) {
                    Some(&*prototype.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if prototype.is_none() {
                prototype = Some(value.get_prototype());
            }

            let method = prototype
                .unwrap()
                .get_method(&property)
                .ok_or(LanguageError::with_source(RuntimeError::NoSuchProperty(property), 0, 0))?;

            Ok(ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction {
                func: method,
                instance: Arc::new(value),
            })))
        }
        Expression::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                Ok(value.clone())
            } else if let LiteralExpressionKind::Object(entries) = r#type {
                let entries: HashMap<String, ValueHolder> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), eval_expr(v, context_ref.clone()).unwrap()))
                    .collect();
                let object_ref = ObjectRef::new(entries, None, context_ref.clone());
                return Ok(ValueHolder::Object(object_ref));
            } else if let LiteralExpressionKind::Array(items) = r#type {
                let items: Vec<ValueHolder> = items
                    .iter()
                    .map(|v| eval_expr(v, context_ref.clone()).unwrap())
                    .collect();
                // ArrayRef creation to be implemented
                return Ok(ValueHolder::Array(ArrayRef::new(items, context_ref.clone()))); // Placeholder
            } else if let LiteralExpressionKind::Variable = r#type {
                return Err(LanguageError::with_source(RuntimeError::VariableNotFound(value.to_string()), 0, 0));
            } else {
                unreachable!();
            }
        }
        Expression::Unary { left, operator } => {
            if let box Expression::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::Number(mut number) } = *left {
                if *operator == TokenKind::Minus {
                    number = number * DynamicNumber::new(NumberHolder::Integer8(-1));

                    return Ok(ValueHolder::Number(number))
                }
            } else {
                return Err(LanguageError::from(RuntimeError::Custom("Unable to use unary operator with this type".to_string())))
            }

            eval_expr(left, context_ref)
        }
        Expression::Variable(variable) => {
            let value = context_ref.borrow().environment.get(variable).ok_or(
                LanguageError::with_source(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    variable.slot, variable.depth
                )), 0, 0),
            )?;

            match value {
                ValueHolder::LazyRef { slot, module: source } => {
                    let program_ref = context_ref.borrow().program.clone();
                    let program = program_ref.borrow();

                    let is_loaded = program.modules.get(&source).unwrap().lock().is_loaded();

                    if !is_loaded {
                        let module = program.modules.get(&source).unwrap().clone();
                        drop(program);
                        Module::execute(module)?
                    } else {
                        drop(program);
                    }

                    let program = program_ref.borrow();

                    let Some(module) = program.modules.get(&source).map(|module| module.lock()) else {
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
            operator,
            right,
            is_definition,
        } => {
            macro_rules! compute_value {
                ($right:expr, $left_block:block) => {
                    if *operator == TokenKind::Assign {
                        Ok($right)
                    } else {
                        let prototype = $right.get_prototype();
                        let operation = match *operator {
                            TokenKind::PlusEqual => Operation::Addition,
                            TokenKind::MinusEqual => Operation::Substraction,
                            TokenKind::AsteriskEqual => Operation::Multiplication,
                            TokenKind::SlashEqual => Operation::Division,
                            TokenKind::PercentEqual => Operation::Modulo,
                            _ => unreachable!()
                        };
                        prototype.operate(operation, &$left_block, &$right)
                    }
                };
            }

            if let Expression::Variable(var_ref) = &**left {
                let expr = eval_expr(right, context_ref.clone())?;
                let value = compute_value!(expr, {
                    let context_ref = context_ref.borrow();
                    context_ref.environment.get(var_ref).unwrap()
                })?;

                context_ref.borrow_mut().environment.set(
                    var_ref,
                    value,
                    *is_definition
                )?;

                return Ok(ValueHolder::Void);
            } else if let Expression::Member { object, property } = &**left {
                let value = eval_expr(&object, context_ref.clone())?;

                if let ValueHolder::Object(ObjectRef { object_id }) = value {
                    let value = eval_expr(property, context_ref.clone())?;

                    let ValueHolder::String(property) = value else {
                        return Err(LanguageError::with_source(RuntimeError::InvalidType(
                            "Property should be a variable".to_string(),
                        ), 0, 0));
                    };

                    let object_ref = ObjectRef { object_id };
                    let expr = eval_expr(right, context_ref.clone())?;
                    let value = compute_value!(expr, {
                        object_ref.get(property.to_string(), context_ref.clone())?
                    })?;
                    object_ref.set(property.to_string(), value, context_ref.clone());

                    return Ok(ValueHolder::Void);
                } else if let ValueHolder::Array(array_ref) = value {
                    let index = match &**property {
                        Expression::Literal { r#type: _, value } => value,
                        Expression::Variable(_) => &eval_expr(&property, context_ref.clone())?,
                        _ => {
                            return Err(LanguageError::with_source(RuntimeError::InvalidType(
                                "Index should be a variable".to_string(),
                            ), 0, 0));
                        }
                    };

                    let index = match index {
                        ValueHolder::Number(f) => <DynamicNumber as Into<usize>>::into(*f),
                        _ => {
                            return Err(LanguageError::with_source(RuntimeError::InvalidType(
                                "Index should be an integer".to_string(),
                            ), 0, 0));
                        }
                    };

                    let expr = eval_expr(right, context_ref.clone())?;
                    let value = compute_value!(expr, {
                        array_ref.get(index, context_ref.clone())?
                    })?;
                    array_ref.set(index, value, context_ref.clone());

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
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = &eval_expr(left, context.clone())?;
    let right = &eval_expr(right, context)?;
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
    callee: &Expression,
    call_arguments: &Vec<Expression>,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let evaluated_args: Vec<ValueHolder> = call_arguments
        .into_iter()
        .map(|arg| eval_expr(&arg, context.clone()))
        .collect::<LanguageResult<Vec<_>>>()?;

    if let Expression::Literal {
        value: ValueHolder::String(name),
        ..
    } = callee
    {
        let table = FUNCTION_TABLE.lock();
        let func = table.get(name.as_str()).cloned();

        drop(table);

        if let Some(func) = func {
            return func(&evaluated_args, context.clone());
        };
    }

    let expr = eval_expr(callee, context.clone())?;

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
            &VariableRef {
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
                .set(argument, value.clone(), true)?;
        }

        let return_value = eval_body(&statements, context.clone());
        context.borrow_mut().environment.exit_scope();
        return return_value;
    }

    if let ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction { func, instance })) = expr {
        return func.call(&instance, evaluated_args, context.clone());
    }

    if let ValueHolder::ClassDefinition(definition) = expr {
        let mut entries = HashMap::new();
        for field in definition.fields {
            entries.insert(field.name, field.value);
        }
        let prototype = Rc::new(definition.prototype);
        let class = ValueHolder::Object(ObjectRef::new(entries, Some(prototype.clone()), context.clone()));
        let constructor = prototype.get_method(&prototype._name);
        if let Some(method) = constructor {
            method.call(&class, evaluated_args, context.clone())?;
        } else if evaluated_args.len() != 0 {
            return Err(LanguageError::with_source(RuntimeError::Custom("No arguments accepted when a constructor is not defined".to_string()), 0 ,0));
        }
        return Ok(class);
    }

    panic!("Tried to call invalid function expression: {:?}", *callee);
}

fn eval_equality(
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(left, context.clone())?;
    let right = eval_expr(right, context)?;

    match operator {
        TokenKind::EQ => Ok(ValueHolder::Bool(left == right)),
        TokenKind::NE => Ok(ValueHolder::Bool(left != right)),
        _ => Err(LanguageError::with_source(RuntimeError::Custom(
            "Invalid equality operator".to_string(),
        ), 0 ,0)),
    }
}

fn eval_relational(
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(left, context.clone())?;
    let right = eval_expr(right, context)?;

    Ok(ValueHolder::Bool(match operator {
        TokenKind::GT => left > right,
        TokenKind::GTE => left >= right,
        TokenKind::LT => left < right,
        TokenKind::LTE => left <= right,
        _ => unreachable!(),
    }))
}

fn eval_logical(
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: Rc<RefCell<ModuleContext>>,
) -> RuntimeResult {
    let left = eval_expr(left, context.clone())?;

    match operator {
        TokenKind::And => {
            let left: bool = left.into();

            if !left {
                return Ok(ValueHolder::Bool(false));
            }

            let right = eval_expr(right, context)?;

            Ok(ValueHolder::Bool(left.into() && right.into()))
        },
        TokenKind::Or => {
            let left: bool = left.into();

            if left {
                return Ok(ValueHolder::Bool(true));
            }

            let right = eval_expr(right, context)?;

            Ok(ValueHolder::Bool(left.into() || right.into()))
        },
        _ => Err(LanguageError::with_source(RuntimeError::Custom(
            "Invalid relational operator".to_string(),
        ), 0 ,0)),
    }
}