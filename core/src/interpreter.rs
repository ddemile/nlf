use core::panic;
use std::{cell::RefCell, collections::HashMap, fmt::{self, Debug}, rc::{Rc, Weak}, sync::Arc};

use indexmap::{IndexMap, IndexSet};
use parking_lot::{Mutex, MutexGuard};
use serde::Serialize;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};
use smallvec::SmallVec;

use crate::{
    errors::{LanguageError, LanguageErrorTrait, LanguageResult}, interpreter::{format::FormatOptions, prototypes::{
        ARRAY_PROTOTYPE, LocalMethodFunc, LocalPrototype, Method, NUMBER_PROTOTYPE, OBJECT_PROTOTYPE, Operation, Prototype, STRING_PROTOTYPE
    }}, lexer::TokenKind, loader::Module, parser::{
        ArrayRef, Block, BuiltInFunction, Expression, ExpressionKind, FunctionKind, IRProgram, IRStatement, IRStatementKind, LiteralExpressionKind, ObjectRef, RuntimeFunction, StatementKind, StatementKindWrapper, ValueHolder, VariableRef, Visibility
    }, stdlib::FUNCTION_TABLE
};

use inline_colorization::*;

pub mod prototypes;
pub mod format;

#[derive(Debug, Serialize, Clone)]
pub struct Field {
    visibility: Visibility,
    name: String,
    value: ValueHolder
}

#[derive(Serialize, Clone)]
pub struct ClassDefinition {
    fields: Vec<Field>,
    #[serde(skip)]
    prototype: LocalPrototype,
    #[serde(skip)]
    methods: HashMap<String, (LocalMethodFunc, Rc<RefCell<Scope>>)>
}

impl Debug for ClassDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassDefinition")
            .field("fields", &self.fields)
            .finish()
    }
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
    pub fn get_prototype(&self) -> &dyn Prototype {
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
            ValueHolder::Object(object_ref) => format::format_object(object_ref, FormatOptions {
                space: if object_ref.fetch().len() > 1 { Some(2) } else { None }
            }),
            ValueHolder::Array(array_ref) => format::format_array(array_ref, FormatOptions {
                space: if array_ref.fetch().len() > 1 { Some(2) } else { None }
            }),
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
    pub schemas: Vec<Schema>
}

impl ProgramContext {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            schemas: vec![]
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
    pub self_rc: Weak<RefCell<ModuleContext>>
}

impl ModuleContext {
    pub fn new(program: Rc<RefCell<ProgramContext>>) -> Rc<RefCell<Self>> {
        let context_ref = Rc::new_cyclic(|weak| {
            RefCell::new(ModuleContext {
                environment: Environment::new(),
                program,
                self_rc: weak.clone(),
            })
        });

        context_ref
    }
}

#[derive(Clone, Debug)]
pub struct Schema {
    id: usize,
    keys: IndexSet<String>,
}

#[derive(Clone)]
pub struct Object {
    pub(crate) schema_id: usize,
    pub(crate) values: Vec<ValueHolder>,
    pub(crate) prototype: Option<Rc<dyn Prototype>>
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .finish()
    }
}

fn get_schema(keys: IndexSet<String>, context: &mut ModuleContext) -> Schema {
    let schema =  context
        .program
        .borrow()
        .schemas
        .iter()
        .find(|s| s.keys == keys)
        .cloned();
    
    schema.unwrap_or_else(|| {
        let schema: Schema = Schema {
            id: context.program.borrow().schemas.len(),
            keys,
        };
        context.program.borrow_mut().schemas.push(schema.clone());
        schema
    })
}

impl ObjectRef {
    pub fn new(entries: IndexMap<String, ValueHolder>, prototype: Option<Rc<dyn Prototype>>, context: &mut ModuleContext) -> Self {
        let keys: IndexSet<String> = entries.keys().cloned().collect();

        let schema = get_schema(keys, context);

        let object = Object {
            schema_id: schema.id,
            values: entries.values().cloned().collect(),
            prototype
        };

        ObjectRef { object: Rc::new(RefCell::new(object)), program_context: context.program.clone() }
    }

    pub(self) fn get(self, property: String, context: &mut ModuleContext) -> RuntimeResult {
        let program = context.program.borrow();

        let object = self.object.borrow();

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

    pub(self) fn set(&self, property: String, value: ValueHolder, context: &mut ModuleContext) {
        let program = context.program.borrow();

        let mut object = self.object.borrow_mut();

        let schema = program
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let mut keys = schema.keys.clone();
        let values = &mut object.values;

        keys.insert(property.clone());

        if let Some(index) = keys.iter().position(|key| *key == property) {
            if index >= values.len() {
                values.push(value);
            } else {
                values[index] = value;
            }
        }

        drop(program);
        
        let schema = get_schema(keys, context);

        object.schema_id = schema.id;
    }

    pub fn fetch(&self) -> IndexMap<String, ValueHolder> {
        let program = self.program_context.borrow();

        let object = self.object.borrow();

        let schema = program
            .schemas
            .iter()
            .find(|schema| schema.id == object.schema_id)
            .expect("Schema not found");

        let mut map = IndexMap::new();

        for i in 0..schema.keys.len() {
            let key = schema.keys[i].clone(); 
            let value = object.values[i].clone();

            map.insert(key, value);
        }

        map
    }

    pub fn get_prototype(&self) -> Option<Rc<dyn Prototype>> {
        let object = self.object.borrow();

        object.prototype.clone()
    }
}

impl ArrayRef {
    pub fn new(items: Vec<ValueHolder>) -> Self {
        let object = Object { schema_id: 0, values: items, prototype: None };

        ArrayRef { object: Rc::new(RefCell::new(object)) }
    }

    pub fn get(&self, index: usize) -> RuntimeResult {
        let object = self.object.borrow();

        let value = object.values.get(index).cloned().ok_or_else(|| LanguageError::from(RuntimeError::Custom(format!(
            "Index {} out of bounds",
            index
        ))))?;

        Ok(value)
    }

    pub fn set(&self, index: usize, value: ValueHolder) {
        let mut object = self.object.borrow_mut();

        let values = &mut object.values;

        if index >= values.len() {
            values.resize(index + 1, ValueHolder::Void);
        } 
        values[index] = value;
    }

    pub fn fetch(&self) -> Vec<ValueHolder> {
        let object = self.object.borrow();

        object.values.clone()
    }

    pub fn push(&self, value: ValueHolder) {
        let mut object = self.object.borrow_mut();

        object.values.push(value);
    }

    pub fn reverse(&self) {
        let mut object = self.object.borrow_mut();

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
    parent: Option<Rc<RefCell<Scope>>>
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub scopes: Vec<Rc<RefCell<Scope>>>
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![]
        }
    }

    pub fn init(&mut self) {
        self.scopes.push(Rc::new(RefCell::new(Scope {
            kind: ScopeKind::Program,
            slots: vec![],
            interrupted: None,
            parent: None
        })));
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Rc::new(RefCell::new(Scope {
            kind,
            slots: vec![],
            interrupted: None,
            parent: self.scopes.last().cloned()
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
                        _ => scope.parent.clone().ok_or_else(|| LanguageError::from(RuntimeError::VariableNotFound(format!(
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
                .ok_or_else(|| LanguageError::from(RuntimeError::VariableNotFound(format!(
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
    program: IRProgram,
    context: &mut ModuleContext,
) -> LanguageResult<()> {
    eval_body(program.body, context)?;

    Ok(())
}

fn hoist_declarations(
    statements: Rc<[IRStatement]>,
    context: &mut ModuleContext,
) -> LanguageResult<()> {
    for statement in statements
        .iter()
        .filter(|statement| matches!(*statement, IRStatement { kind: StatementKind::Function { .. } | StatementKind::Class { .. }, .. } ))
    {
        match &statement.kind {
            IRStatementKind::Function { variable, arguments, block, .. } => {
                let scope = context.environment.scopes.last_mut().cloned().unwrap();
                
                let context_ptr = context as *mut ModuleContext;

                context.environment.set(
                    &variable,
                    ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                        arguments: arguments.to_vec(),
                        statements: block.statements.clone(),
                        scope,
                        context: context_ptr
                    })),
                    true,
                )?
            }
            IRStatementKind::Class { variable, methods: raw_methods, fields: raw_fields, .. } => {
                let mut fields: Vec<Field> = vec![];

                let mut prototype = LocalPrototype::new(variable.name.clone().unwrap().as_ref());

                let mut static_methods: HashMap<String, (LocalMethodFunc, Rc<RefCell<Scope>>)> = HashMap::new();

                for method in raw_methods {
                    let StatementKind::Method { name, arguments, block } = method.kind.clone() else {
                        unreachable!()
                    };

                    let is_constructor = Some(name.clone()) == variable.name;

                    let is_method_static = !arguments
                        .get(0)
                        .and_then(|arg| arg.name.clone())
                        .is_some_and(|name| name == "self") && !is_constructor;
                    
                    let arguments = if is_method_static || is_constructor { arguments } else { arguments[1..].to_vec() };

                    let scope = context.environment.scopes.last_mut().cloned().unwrap();

                    let method = Box::new(move |this: &ValueHolder, call_arguments: &[ValueHolder], context_ref: &mut ModuleContext, scope: Rc<RefCell<Scope>>| {  
                        if arguments.len() != call_arguments.len() {
                            return Err(LanguageError::with_source(RuntimeError::Custom("Invalid number of args".to_string()), 0 ,0));
                        }

                        let context = context_ref;

                        let statements: Rc<[IRStatement]> = block.statements.clone();

                        context
                            .environment
                            .enter_scope(ScopeKind::Call(scope.clone()));

                        let context_ptr = context as *mut ModuleContext;

                        context.environment.set(
                            &VariableRef {
                                name: None,
                                slot: 0,
                                depth: 0,
                                start: 0,
                                end: 0
                            },
                            ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                                arguments: arguments.clone(),
                                statements: statements.clone(),
                                scope: scope.clone(),
                                context: context_ptr
                            })),
                            true,
                        )?;

                        if let ValueHolder::Object(object_ref) = this {
                            if !is_method_static {
                                context.environment.set(
                                    &VariableRef {
                                        name: None,
                                        slot: 1,
                                        depth: 0,
                                        start: 0,
                                        end: 0
                                    },
                                    ValueHolder::Object(object_ref.clone()),
                                    true,
                                )?
                            }
                        }

                        for (argument, value) in arguments.iter().zip(call_arguments.iter()) {
                            context
                                .environment
                                .set(argument, value.clone(), true)?;
                        }

                        let return_value = eval_body(statements.clone(), context);
                        context.environment.exit_scope();
                        return_value
                    });

                    if is_method_static {
                        static_methods.insert(name.clone(), (method, scope));
                        continue;
                    }
                    
                    prototype.with_method(&name, method, scope);
                }

                for field in raw_fields {
                    let StatementKind::Field { visibility, name, value } = field.kind.clone() else {
                        unreachable!()
                    };

                    fields.push(Field {
                        visibility: visibility,
                        name: name,
                        value: eval_expr(&value, context)?
                    });
                }
                
                context.environment.set(
                    variable,
                    ValueHolder::ClassDefinition(ClassDefinition {
                        fields,
                        prototype,
                        methods: static_methods
                    }),
                    true,
                )?
            }
            _=> ()
        }
    }

    Ok(())
}

pub fn eval_body(statements: Rc<[IRStatement]>, context: &mut ModuleContext) -> RuntimeResult {
    hoist_declarations(statements.clone(), context)?;

    for statement in statements.iter() {
        let value = eval_statement(statement, context)?;
        if let Some(value) = &context.environment.scopes.last().unwrap().borrow().interrupted {
            return Ok(value.clone());
        }

        for mut scope in context.environment.scopes.iter().rev().map(|scope| scope.borrow_mut()) {
            match (&statement.kind, &scope.kind) {
                (IRStatementKind::Return { .. }, ScopeKind::Call(_)) => {
                    scope.interrupted = Some(value.clone());

                    return Ok(value);
                }
                (_, ScopeKind::Call(_)) => {
                    break;
                }
                (IRStatementKind::Break, ScopeKind::Loop) => {
                    scope.interrupted = Some(ValueHolder::Void);

                    return Ok(value);
                }
                _ => (),
            }
        }
    }

    Ok(ValueHolder::Void)
}

fn eval_statement(statement: &IRStatement, context: &mut ModuleContext) -> RuntimeResult {
    match &statement.kind {
        IRStatementKind::Expression { expression } => eval_expr(&expression, context),
        IRStatementKind::If {
            condition,
            block,
            alternate,
        } => eval_if(&condition, block, alternate, context),
        IRStatementKind::For {
            variable,
            left,
            right,
            statements,
        } => eval_for(variable, left, right, statements.clone(), context),
        IRStatementKind::While {
            condition,
            statements,
        } => eval_while(&condition, statements.clone(), context),
        IRStatementKind::Function {
            ..
        } => Ok(ValueHolder::Void),
        IRStatementKind::Return { expression } => eval_expr(&expression, context),
        IRStatementKind::Break => Ok(ValueHolder::Void),
        IRStatementKind::Block(_) => Ok(ValueHolder::Void),
        IRStatementKind::Import { .. } => Err(LanguageError::with_source(RuntimeError::Custom(
            "Import declarations can only be at the top of modules".into(),
        ), 0, 0)),
        IRStatementKind::Export { declaration } => eval_statement(declaration, context),
        IRStatementKind::Class { .. } => Ok(ValueHolder::Void),
        IRStatementKind::VariableDefinition { descriptor, expression, .. } => eval_definition(descriptor, expression, context),
        _ => panic!("Invalid statement : {:?}", statement),
    }
}

fn eval_for(
    variable: &VariableRef,
    left: &Option<Expression>,
    right: &Option<Expression>,
    statements: Rc<[IRStatement]>,
    context: &mut ModuleContext,
) -> RuntimeResult {
    let mut iter: Box<dyn Iterator<Item = i32>> = Box::new(0..);

    match (left, right) {
        (Some(_), Some(_)) => {
            let left: i32 = eval_expr(left.as_ref().unwrap(), context)?.into();
            let right: i32 = eval_expr(right.as_ref().unwrap(), context)?.into();

            iter = if left < right {
                Box::new(left..right)
            } else {
                Box::new(((right + 1)..(left + 1)).rev())
            };
        }
        (Some(_), None) => {
            let left: i32 = eval_expr(left.as_ref().unwrap(), context)?.into();

            iter = Box::new(left..)
        }
        (None, Some(_)) => {
            let right: i32 = eval_expr(right.as_ref().unwrap(), context)?.into();

            iter = Box::new(0..right)
        }
        _ => (),
    }

    context.environment.enter_scope(ScopeKind::Loop);
    context
        .environment
        .set(variable, ValueHolder::Number(DynamicNumber::new(NumberHolder::Integer8(0))), true)?;

    for i in iter {
        {
            // Faster than environment.set in this context
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

            scope.slots.fill(ValueHolder::Void);

            scope.slots[variable.slot] = ValueHolder::Number(DynamicNumber::new(NumberHolder::Integer32(i)));
        }

        eval_body(statements.clone(), context)?;
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
    condition: &Expression,
    statements: Rc<[IRStatement]>,
    context: &mut ModuleContext,
) -> RuntimeResult {
    context.environment.enter_scope(ScopeKind::Loop);

    fn execute_loop_body(
        statements: Rc<[IRStatement]>,
        context: &mut ModuleContext
    ) -> LanguageResult<bool> {
        {
            let mut scope = context.environment.scopes.last().unwrap().borrow_mut();

            scope.slots.fill(ValueHolder::Void);
        }

        eval_body(statements, context)?;
        let broken = context
            .environment
            .scopes
            .last()
            .unwrap()
            .borrow()
            .interrupted
            .is_some();

        return Ok(broken)
    }

    let optimized_condition = if let ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::Bool(value) } = condition.kind {
        Some(value)
    } else {
        None
    };

    if let Some(value) = optimized_condition {
        if value {
            loop {
                if execute_loop_body(statements.clone(), context)? {
                    break;
                }
            }
        }
    } else {
        while eval_expr(condition, context)?.into() {
            if execute_loop_body(statements.clone(), context)? {
                break;
            }
        }
    }

    context.environment.exit_scope();

    Ok(ValueHolder::Void)
}

fn eval_if(
    condition: &Expression,
    block: &Block<IRStatement>,
    alternate: &Option<Box<IRStatement>>,
    context: &mut ModuleContext,
) -> RuntimeResult {
    let cond: bool = eval_expr(condition, context)?.into();

    if cond {
        context.environment.enter_scope(ScopeKind::Regular);
        eval_body(block.statements.clone(), context)?;
        context.environment.exit_scope();
    } else if let Some(box IRStatement { kind: IRStatementKind::If {
        condition,
        block,
        alternate,
    }, .. }) = alternate
    {
        eval_if(&condition, block, alternate, context)?;
    } else if let Some(box IRStatement { kind : IRStatementKind::Block(Block { statements, .. }), .. }) = alternate {
        context.environment.enter_scope(ScopeKind::Regular);
        eval_body(statements.clone(), context)?;
        context.environment.exit_scope();
    }
    Ok(ValueHolder::Void)
}

fn eval_definition(variables: &Vec<VariableRef>, expression: &Expression, context: &mut ModuleContext) -> RuntimeResult {
    // TODO: use proper values

    let value = eval_expr(expression, context)?;

    for variable in variables {
        context.environment.set(
            variable,
            value.clone(),
            true
        )?;
    }

    Ok(ValueHolder::Void)
}

fn eval_expr(expr: &Expression, context: &mut ModuleContext) -> RuntimeResult {
    return match &expr.kind {
        ExpressionKind::Binary {
            left,
            operator,
            right,
        } => eval_binary(&left, &operator, &right, context),
        ExpressionKind::Member { object, property } => {
            let value = eval_expr(&object, context)?;

            let property = eval_expr(&property, context)?;

            if matches!(&property, ValueHolder::Number(_)) {
                let index = match &property {
                    ValueHolder::Number(f) => <DynamicNumber as Into<usize>>::into(*f),
                    _ => unreachable!()
                };

                if let ValueHolder::Array(array_ref) = &value {
                    return array_ref.get(index);
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
                match object.clone().get(property.to_string(), context) {
                    Ok(object) => return Ok(object),
                    Err(_) => (),
                };
            };

            if let ValueHolder::ClassDefinition(definition) = &value {
                if let Some((method, scope)) = definition.methods.get(&property.to_string()) {
                    return Ok(ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction {
                        func: Method::Local(method.clone(), scope.clone()),
                        instance: Rc::new(ValueHolder::Void),
                    })));
                }
            }

            let mut prototype: Option<&dyn Prototype> = if let ValueHolder::Object(ref object) = value {
                if let Some(prototype) = object.get_prototype() {
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
                .ok_or_else(|| LanguageError::with_source(RuntimeError::NoSuchProperty(property), 0, 0))?;

            Ok(ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction {
                func: method,
                instance: Rc::new(value),
            })))
        }
        ExpressionKind::Literal { r#type, value } => {
            if let LiteralExpressionKind::Literal = r#type {
                Ok(value.clone())
            } else if let LiteralExpressionKind::Object(entries) = r#type {
                let entries: IndexMap<String, ValueHolder> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), eval_expr(v, context).unwrap()))
                    .collect();
                let object_ref = ObjectRef::new(entries, None, context);
                return Ok(ValueHolder::Object(object_ref));
            } else if let LiteralExpressionKind::Array(items) = r#type {
                let items: Vec<ValueHolder> = items
                    .iter()
                    .map(|v| eval_expr(v, context).unwrap())
                    .collect();
                // ArrayRef creation to be implemented
                return Ok(ValueHolder::Array(ArrayRef::new(items))); // Placeholder
            } else if let LiteralExpressionKind::Function(statement) = r#type {
                let box StatementKindWrapper::IR(IRStatementKind::Function { variable: _, arguments, block, .. }) = statement.clone() else {
                    panic!()
                };

                let scope = context.environment.scopes.last_mut().cloned().unwrap();

                return Ok(ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
                    arguments,
                    statements: block.statements,
                    scope,
                    context,
                })))
            } else if let LiteralExpressionKind::Variable = r#type {
                return Err(LanguageError::with_source(RuntimeError::VariableNotFound(value.to_string()), 0, 0));
            } else {
                unreachable!();
            }
        }
        ExpressionKind::Unary { left, operator } => {
            let value = eval_expr(left, context)?;
            match (value, operator) {
                (ValueHolder::Number(mut number), TokenKind::Minus) => {
                    number = number * DynamicNumber::new(NumberHolder::Integer8(-1));

                    return Ok(ValueHolder::Number(number))
                },
                (ValueHolder::Bool(mut boolean), TokenKind::Bang) => {
                    boolean = !boolean;

                    return Ok(ValueHolder::Bool(boolean))
                },
                _ => return Err(LanguageError::with_source(RuntimeError::Custom("Unable to use this unary operator with this type".to_string()), left.start - 1, left.start))
            }
        }
        ExpressionKind::Variable(variable) => {
            let value = context.environment.get(&variable).ok_or_else(|| {
                LanguageError::with_source(RuntimeError::VariableNotFound(format!(
                    "Slot {} at depth {} not found",
                    variable.slot, variable.depth
                )), 0, 0)
            })?;

            match value {
                ValueHolder::LazyRef { slot, module: source } => {
                    let program_ref = context.program.clone();
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
        ExpressionKind::Assignment {
            left,
            operator,
            right
        } => {
            macro_rules! compute_value {
                ($right:expr, $left_block:block) => {
                    if *operator == TokenKind::Assign {
                        Ok($right)
                    } else {
                        let prototype = $right.get_prototype();
                        let operation = match operator {
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

            if let ExpressionKind::Variable(var_ref) = &left.kind {
                let expr = eval_expr(&right, context)?;
                let value = compute_value!(expr, {
                    context.environment.get(var_ref).unwrap()
                })?;

                context.environment.set(
                    var_ref,
                    value.clone(),
                    false
                )?;

                return Ok(value);
            } else if let ExpressionKind::Member { object, property } = &left.kind {
                let value = eval_expr(&object, context)?;

                if let ValueHolder::Object(object_ref) = value {
                    let value = eval_expr(property, context)?;

                    let ValueHolder::String(property) = value else {
                        return Err(LanguageError::with_source(RuntimeError::InvalidType(
                            "Property should be a variable".to_string(),
                        ), 0, 0));
                    };

                    let expr = eval_expr(&right, context)?;
                    let value = compute_value!(expr, {
                        object_ref.clone().get(property.to_string(), context)?
                    })?;
                    object_ref.set(property.to_string(), value, context);

                    return Ok(ValueHolder::Void);
                } else if let ValueHolder::Array(array_ref) = value {
                    let index = match &property.kind {
                        ExpressionKind::Literal { r#type: _, value } => value,
                        ExpressionKind::Variable(_) => &eval_expr(&property, context)?,
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

                    let expr = eval_expr(&right, context)?;
                    let value = compute_value!(expr, {
                        array_ref.get(index)?
                    })?;
                    array_ref.set(index, value);

                    return Ok(ValueHolder::Void);
                } else {
                    return Err(LanguageError::with_source(RuntimeError::InvalidType(
                        "Cannot access property on type other than Object".to_string(),
                    ), 0, 0));
                }
            }

            panic!("Expected variable for assignment got {:?}", *left);
        }
        ExpressionKind::Call { callee, arguments } => eval_call(callee, arguments, context),
        ExpressionKind::Equality {
            left,
            operator,
            right,
        } => eval_equality(left, operator, right, context),
        ExpressionKind::Relational {
            left,
            operator,
            right,
        } => eval_relational(left, operator, right, context),
        ExpressionKind::Logical {
            left,
            operator,
            right,
        } => eval_logical(left, operator, right, context),
    };
}

fn eval_binary(
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: &mut ModuleContext,
) -> RuntimeResult {
    let left = &eval_expr(left, context)?;
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
    context: &mut ModuleContext,
) -> RuntimeResult {
    let mut evaluated_args: SmallVec<[ValueHolder; 4]> = SmallVec::new();
    for arg in call_arguments {
        evaluated_args.push(eval_expr(arg, context)?);
    }

    if let ExpressionKind::Literal {
        value: ValueHolder::String(name),
        ..
    } = &callee.kind
    {
        let table = FUNCTION_TABLE.lock();
        let func = table.get(name.as_str()).cloned();

        drop(table);

        if let Some(func) = func {
            return func(&evaluated_args, context);
        };
    }

    let expr = eval_expr(callee, context)?;

    if let ValueHolder::Fn(FunctionKind::Runtime(function)) = expr
    {
        return eval_runtime_function(function, &evaluated_args, context);
    }

    if let ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction { func, instance })) = expr {
        return func.call(&instance, &evaluated_args, context);
    }

    if let ValueHolder::ClassDefinition(definition) = expr {
        let mut entries = IndexMap::new();
        for field in definition.fields {
            entries.insert(field.name, field.value);
        }
        let prototype = Rc::new(definition.prototype);
        let class = ValueHolder::Object(ObjectRef::new(entries, Some(prototype.clone()), context));
        let constructor = prototype.get_method(&prototype._name);
        if let Some(method) = constructor {
            method.call(&class, &evaluated_args, context)?;
        } else if evaluated_args.len() != 0 {
            return Err(LanguageError::with_source(RuntimeError::Custom("No arguments accepted when a constructor is not defined".to_string()), 0 ,0));
        }
        return Ok(class);
    }

    panic!("Tried to call invalid function expression: {:?}", *callee);
}

pub fn eval_runtime_function(function: RuntimeFunction, evaluated_args: &[ValueHolder], context: &mut ModuleContext) -> RuntimeResult {
    if function.arguments.len() != evaluated_args.len() {
        return Err(LanguageError::with_source(RuntimeError::Custom("Invalid number of args".to_string()), 0 ,0));
    }

    context
        .environment
        .enter_scope(ScopeKind::Call(function.scope.clone()));

    let context_ptr = context as *mut ModuleContext;

    context.environment.set(
        &VariableRef {
            name: None,
            slot: 0,
            depth: 0,
            start: 0,
            end: 0
        },
        ValueHolder::Fn(FunctionKind::Runtime(RuntimeFunction {
            arguments: function.arguments.clone(),
            statements: function.statements.clone(),
            scope: function.scope,
            context: context_ptr
        })),
        true,
    )?;

    for (argument, value) in function.arguments.iter().zip(evaluated_args.iter()) {
        context
            .environment
            .set(argument, value.clone(), true)?;
    }

    let return_value = eval_body(function.statements, context);
    context.environment.exit_scope();
    return return_value;
}

fn eval_equality(
    left: &Expression,
    operator: &TokenKind,
    right: &Expression,
    context: &mut ModuleContext,
) -> RuntimeResult {
    let left = eval_expr(left, context)?;
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
    context: &mut ModuleContext,
) -> RuntimeResult {
    let left = eval_expr(left, context)?;
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
    context: &mut ModuleContext,
) -> RuntimeResult {
    let left = eval_expr(left, context)?;

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