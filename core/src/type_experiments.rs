use core::panic;
use std::{collections::HashMap, fmt::Debug, fs, marker::PhantomData, rc::Rc, sync::Arc};

use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};

use crate::{analysis::{Span, Symbol, SymbolKind}, errors::TypingError, parser::{ASTBlock, ASTProgram, ASTStatement, ASTSyntaxTree, Argument, Block, Identifier, Program, Statement, StatementKind, TyStatement, TyStatementKind, TypeId, TypeRef, Visibility}};

#[derive(Debug)]
struct InternalArena<T: Clone, K: Into<usize> + From<usize>> {
    data: Vec<T>,
    key_ty: PhantomData<K>
}

pub struct Arena<T: Clone, K: Into<usize> + From<usize>>(Arc<Mutex<InternalArena<T, K>>>);

impl<T: Clone, K: Into<usize> + From<usize>> Arena<T, K> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InternalArena { data: vec![], key_ty: PhantomData })))
    }

    pub fn alloc(&self, data: T) -> K {
        let mut arena = self.0.lock();
        let id = arena.data.len();
        arena.data.push(data);
        K::from(id)
    }

    pub fn get(&self, id: K) -> T {
        self.0.lock().data[id.into()].clone()
    }

    pub fn get_mut(&self, id: K) -> MappedMutexGuard<'_, T> {
        let guard = self.0.lock();
        MutexGuard::map(guard, |arena| &mut arena.data[id.into()])
    }

    /// Returns a copy of all the data that this arena allocated in the form of a `Vec<T>`
    pub fn snapshot(&self) -> Vec<T> {
        self.0.lock().data.to_vec()
    }
}

type TypeArena = Arena<AbstractType, TypeId>;

pub struct InternalTypeAllocator<T> {
    pub data: Vec<Vec<T>>
}

#[derive(Clone)]
pub struct TypeAllocator<T>(Arc<Mutex<InternalTypeAllocator<T>>>);

impl<T> TypeAllocator<T> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InternalTypeAllocator { data: vec![] })))
    }

    pub fn get_mut(&self, id: TypeId) -> MappedMutexGuard<'_, T> {
        let guard = self.0.lock();
        MutexGuard::map(guard, |arena| &mut arena.data[id.0 as usize][id.1 as usize])
    }
}

#[derive(Clone)]
pub struct NamespaceHandle<T>(i32, TypeAllocator<T>);

impl<T> NamespaceHandle<T> {
    pub fn recreate(&self) {
        self.1.0.lock().data[self.0 as usize] = vec![];
    }

    pub fn alloc(&self, data: T) -> TypeId {
        let allocator = &mut self.1.0.lock().data;
        let arena = &mut allocator[self.0 as usize];
        let id = arena.len();
        arena.push(data);
        TypeId(self.0, id as u32)
    }

    pub fn allocator(&self) -> TypeAllocator<T> where T: Clone {
        self.1.clone()
    }
}

impl<T: Clone> TypeAllocator<T> {
    pub fn create_namespace(&mut self) -> NamespaceHandle<T> {
        let id = self.0.lock().data.len() as i32;
        self.0.lock().data.push(vec![]);
        NamespaceHandle(id, self.clone())
    }
}

impl Into<usize> for TypeId {
    fn into(self) -> usize {
        self.1 as usize
    }
}

impl From<usize> for TypeId {
    fn from(value: usize) -> Self {
        Self(-1, value as u32)
    }
}

impl TypeArena {
    pub fn debug(&self) -> String {
        format!("{:#?}", self.0.lock())
    }

    pub fn debug_class(&self) -> String {
        for ty in self.snapshot() {
            if ty.name == "Test" {
                return format!("{:#?}", ty);
            }
        }

        String::new()
    }
}

#[derive(Debug, Clone)]
pub enum AbstractTypeKind {
    Interface,
    Alias(TypeId),
    Function,
    Class,
    Variable,
    BuiltIn,
    Import,
    Unresolved(TypeRef)
}

#[derive(Debug, Clone)]
pub struct AbstractType {
    pub name: String,
    pub kind: AbstractTypeKind,
    pub span: Span
}

fn collect_types(statements: &[ASTStatement], collector: &TypeCollector) -> HashMap<String, TypeId> {
    let mut types = HashMap::new();

    for statement in statements {
        match &statement.kind {
            StatementKind::Class { variable, .. } => {
                let r#type = AbstractType {
                    name: variable.value.clone(),
                    kind: AbstractTypeKind::Class,
                    span: Span { start: variable.start, end: variable.end }
                };

                types.insert(r#type.name.clone(), collector.types.alloc(r#type));
            }
            StatementKind::TypeAlias { ident, type_ref, .. } => {
                let r#type = AbstractType {
                    name: ident.value.clone(),
                    kind: AbstractTypeKind::Unresolved(type_ref.clone()),
                    span: ident.get_span()
                };

                types.insert(r#type.name.clone(), collector.types.alloc(r#type));
            }
            _ => {}
        }
    }

    types
}

pub struct Frame {
    pub types: HashMap<String, TypeId>
}

#[derive(Debug, Clone)]
pub struct SymbolFrame {
    pub span: Span,
    pub symbols: Vec<Symbol>,
    pub children: Vec<SymbolFrame>
}

impl SymbolFrame {
    pub fn insert(&mut self, frame: SymbolFrame) -> bool {
        if frame.span.start < self.span.start && frame.span.end > self.span.end {
            return false
        }

        for child in self.children.iter_mut() {
            if child.insert(frame.clone()) {
                return true;
            }
        }

        self.children.push(frame);

        true
    }

    pub fn insert_symbol(&mut self, symbol: Symbol) -> bool {
        if symbol.span.start < self.span.start && symbol.span.end > self.span.end {
            return false
        }

        for child in self.children.iter_mut() {
            if child.insert_symbol(symbol.clone()) {
                return true;
            }
        }

        self.symbols.push(symbol);

        true
    }

    pub fn find_symbol(&self, position: usize) -> Option<Symbol> {
        if position < self.span.start && position > self.span.end {
            return None
        }

        for child in self.children.iter() {
            if let Some(symbol) =  child.find_symbol(position) {
                return Some(symbol);
            }
        }

        for symbol in self.symbols.iter() {
            if position >= symbol.span.start && position <= symbol.span.end {
                return Some(symbol.clone())
            }
        }

        None
    }
}

pub trait StringifyType {
    fn stringify(&self, arena: &TypeArena) -> String;
}

pub struct AliasType {
    pub name: String,
    pub ty: TypeId
}

// impl StringifyType for AliasType {
//     fn stringify(&self, arena: &TypeArena) -> String {
//         // format!("type {} = {}", self.name);
//     }
// }

pub struct ClassField {
    pub name: String,
    pub ty: TypeId,
    pub visibility: Visibility
}

pub struct ClassMethod {
    pub name: String,
    pub return_ty: TypeId,
    pub return_type_ref: TypeRef
}

pub struct ClassType {
    pub name: String,
    pub fields: Vec<ClassField>,
    pub methods: Vec<ClassMethod>
}

pub enum Type {
    
}

pub struct TypeCollector {
    pub types: NamespaceHandle<AbstractType>,
    pub type_frames: Vec<Frame>,
    pub symbol_tree: SymbolFrame,
    resolved_types: HashMap<TypeId, Type>
}

impl TypeCollector {
    pub fn find_type(&self, name: &str) -> Option<TypeId> {
        for frame in self.type_frames.iter().rev() {
            if let Some(type_id) = frame.types.get(name) {
                return Some(*type_id);
            }
        }
        None
    }

    pub fn resolve_builtin(&self, name: &str) -> Option<TypeId> {
        self.type_frames.first().unwrap().types.get(name).cloned()
    }

    pub fn enter_frame(&mut self, types: HashMap<String, TypeId>) {
        self.type_frames.push(Frame { types });
    }

    pub fn leave_frame(&mut self) {
        self.type_frames.pop();
    }
}

pub fn stringify_symbol(symbol: &Symbol, _arena: &mut TypeArena) -> String {
    match symbol.kind {
        SymbolKind::Alias(type_id) => format!("{} -> {}", symbol.name, type_id.0),
        _ => format!("Not implemented")
    }
}

// pub fn discover_program(program: ASTProgram) -> TypeCollector {
//     let builtins = vec![
//         AbstractType { name: "unknown".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
//         AbstractType { name: "string".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
//         AbstractType { name: "number".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
//         AbstractType { name: "bool".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() }
//     ];

//     let arena = TypeArena::new();
//     let frames = vec![Frame { types: builtins.into_iter().map(|ty| (ty.name.clone(), arena.alloc(ty))).collect() }];

//     let mut collector = TypeCollector {
//         arena,
//         type_frames: frames,
//         symbol_tree: SymbolFrame { span: Span::max(), symbols: vec![], children: vec![] },
//         resolved_types: HashMap::new()
//     };

//     collector.type_frames.push(Frame { types: collect_types(&program.body, &collector) });

//     program.body.iter().for_each(|statement| discover_statement(statement, &mut collector));

//     fs::write("debug.txt", format!("{:#?}", collector.symbol_tree)).unwrap();

//     collector
// }

// pub fn discover_statement(statement: &ASTStatement, collector: &mut TypeCollector) {
//     match &statement.kind {
//         StatementKind::Function { variable, arguments, block, return_type_ref, .. } => {
//             // Store function signature as a symbol
//             let symbol = Symbol {
//                 kind: SymbolKind::Function(FunctionType {
//                     arguments: arguments.iter().map(|argument| Argument { variable: argument.variable.clone(), type_ref: argument.type_ref.clone(), ty: resolve_type_ref(&argument.type_ref, collector).unwrap_or_else(|_| collector.resolve_builtin("unknown").unwrap()) }).collect(),
//                     return_ty: resolve_type_ref(return_type_ref, collector).unwrap_or_else(|_| collector.resolve_builtin("unknowned").unwrap())
//                 }),
//                 name: variable.value.clone(),
//                 scope: ScopeId(0),
//                 span: Span { start: variable.start, end: variable.end }
//             };
            
//             collector.symbol_tree.insert_symbol(symbol);

//             // Collect types and symbols recursively
//             collector.symbol_tree.insert(SymbolFrame { span: block.get_span(), symbols: vec![], children: vec![] });
            
//             collector.type_frames.push(Frame { types: collect_types(&block.statements, &collector) });

//             discover_block(block, collector);

//             collector.type_frames.pop();
//         }
//         StatementKind::VariableDefinition { descriptor: VariableDescriptor::Identifier(ident), type_ref, .. } => {
//             let symbol = Symbol {
//                 kind: SymbolKind::Definition(resolve_type_ref(type_ref, collector).unwrap_or_else(|_| collector.resolve_builtin("unknowned").unwrap())),
//                 name: ident.value.clone(),
//                 span: Span { start: ident.start, end: ident.end },
//                 scope: ScopeId(0)
//             };

//             collector.symbol_tree.insert_symbol(symbol);
//         }
//         StatementKind::TypeAlias { ident, type_ref, .. } => {
//             let alias_id = collector.find_type(&ident.value).unwrap();
//             let type_id = resolve_type_ref(&Some(type_ref.clone()), collector).unwrap_or_else(|_| collector.resolve_builtin("unknowned").unwrap());

//             collector.resolved_types.insert(alias_id, Type::Alias(type_id));

//             let symbol = Symbol {
//                 kind: SymbolKind::Alias(type_id),
//                 name: ident.value.clone(),
//                 span: ident.get_span(),
//                 scope: ScopeId(0)
//             };

//             collector.symbol_tree.insert_symbol(symbol);
//         }
//         StatementKind::Expression { expression } => {

//         }
//         _ => {}
//     }
// }

// fn discover_expression(expression: &Expression, collector: &mut TypeCollector) {
//     match expression.kind {
//         ExpressionKind::Variable(var_ref) => {

//         }
//         _ => {}
//     }
// }

// pub fn discover_block(block: &ASTBlock, collector: &mut TypeCollector) {
//     block.statements.iter().for_each(|statement| discover_statement(statement, collector))
// }

pub fn resolve_type_ref(type_ref: &Option<TypeRef>, collector: &TypeCollector) -> Result<TypeId, TypingError> {
    match type_ref {
        Some(TypeRef::Named(ident)) => collector.find_type(&ident.value).ok_or(TypingError::TypeNotFound(ident.value.clone())),
        _ => Ok(collector.resolve_builtin("unknown").unwrap())
    }
}

struct TypeResolverContext {
    collector: TypeCollector
}

pub struct ProgramContext {
    pub namespace: NamespaceHandle<AbstractType>
}

pub fn type_program(program: ASTProgram, context: &mut ProgramContext) -> Result<(Program<TyStatement>, TypeCollector), TypingError> {
    let builtins = vec![
        AbstractType { name: "unknown".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
        AbstractType { name: "string".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
        AbstractType { name: "number".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() },
        AbstractType { name: "bool".into(), kind: AbstractTypeKind::BuiltIn, span: Span::empty() }
    ];

    let frames = vec![Frame { types: builtins.into_iter().map(|ty| (ty.name.clone(), context.namespace.alloc(ty))).collect() }];

    let mut collector = TypeCollector {
        types: context.namespace.clone(),
        type_frames: frames,
        symbol_tree: SymbolFrame { span: Span::max(), symbols: vec![], children: vec![] },
        resolved_types: HashMap::new()
    };

    collector.enter_frame(collect_types(&program.body, &collector));
    
    let mut context = TypeResolverContext { collector };

    let mut typed_statements = vec![];

    for statement in program.body.iter() {
        typed_statements.push(type_statement(statement, &mut context)?);
    }

    context.collector.leave_frame();

    let typed_program: Program<TyStatement> = Program { body: typed_statements.into() };

    fs::write("debug.txt", format!("{:#?}", context.collector.symbol_tree)).unwrap();

    Ok((typed_program, context.collector))
}

fn type_statement(statement: &ASTStatement, context: &mut TypeResolverContext) -> Result<TyStatement, TypingError> {
    let typed_kind = match &statement.kind {
        StatementKind::TypeAlias { ident, type_ref, .. } => {
            let ty = resolve_type_ref(&Some(type_ref.clone()), &context.collector)?;

            let alias_id = context.collector.type_frames.last().unwrap().types.get(&ident.value).unwrap();

            let type_allocator = context.collector.types.allocator();
            let mut alias = type_allocator.get_mut(*alias_id);

            alias.kind = AbstractTypeKind::Alias(ty);

            StatementKind::TypeAlias { ident: ident.clone(), type_ref: type_ref.clone(), ty }
        },
        StatementKind::Import { specifiers, source } => {
            // This works because imports can only be defined at the top of a file

            let specifiers = specifiers.iter().map(|specifier| {
                let ty = AbstractType {
                    kind: AbstractTypeKind::Import,
                    name: specifier.value.clone(),
                    span: specifier.get_span()
                };

                let ty_id = context.collector.types.alloc(ty);

                context.collector.type_frames.last_mut().unwrap().types.insert(specifier.value.clone(), ty_id);

                (specifier.clone(), ty_id)
            }).collect();

            StatementKind::Import { specifiers, source: source.clone() }
        },
        StatementKind::Function { variable, arguments, block, return_type_ref, .. } => {
            type_method(Method { variable: variable.clone(), arguments: arguments.clone(), block, return_type_ref: return_type_ref.clone() }, context)?
        }
        StatementKind::Class { variable, methods, fields, body_start, body_end } => {
            let mut typed_methods = vec![];
            
            for method in methods {
                if let Statement { kind: StatementKind::Function { variable, arguments, block, return_type_ref, .. }, start, end } = method {
                    let statement_kind = type_method(Method { variable: variable.clone(), arguments: arguments.clone(), block, return_type_ref: return_type_ref.clone() }, context)?;
                    
                    typed_methods.push(statement_kind.into_statement(*start, *end));
                }
            }

            let mut typed_fields = vec![];

            for field in fields {
                if let Statement { kind: StatementKind::Field { visibility, name, value, type_ref, .. }, start, end } = field {
                    let statement_kind: StatementKind<ASTSyntaxTree> = StatementKind::Field { visibility: visibility.clone(), name: name.clone(), value: value.clone(), type_ref: type_ref.clone(), ty: resolve_type_ref(type_ref, &context.collector)? };
                    
                    typed_fields.push(statement_kind.into_statement(*start, *end));
                }
            }

            StatementKind::Class { variable: variable.clone(), methods: typed_methods, fields: typed_fields, body_start: *body_start, body_end: *body_end }
        }
        _ => panic!()
    };

    Ok(typed_kind.into_statement(statement.start, statement.end))
}

struct Method<'a> {
    variable: Identifier,
    arguments: Vec<Argument<Identifier, ()>>,
    block: &'a ASTBlock,
    return_type_ref: Option<TypeRef>
}

fn type_method(method: Method, context: &mut TypeResolverContext) -> Result<TyStatementKind, TypingError> {
    let mut typed_arguments = vec![];

    for argument in method.arguments {
        typed_arguments.push(Argument { variable: argument.variable, type_ref: argument.type_ref.clone(), ty: resolve_type_ref(&argument.type_ref, &context.collector)? });
    }

    let return_ty = resolve_type_ref(&method.return_type_ref, &context.collector)?;

    context.collector.enter_frame(collect_types(&method.block.statements, &context.collector));

    let block = type_block(&method.block, context)?;

    context.collector.leave_frame();
                
    Ok(StatementKind::Function { variable: method.variable, arguments: typed_arguments, block, return_type_ref: method.return_type_ref, return_ty })
}

fn type_block(block: &Block<ASTStatement>, context: &mut TypeResolverContext) -> Result<Block<TyStatement>, TypingError> {
    let mut statements = vec![];

    for statement in block.statements.iter() {
        statements.push(type_statement(statement, context)?);
    }

    Ok(Block { statements: Rc::from(statements), start: block.start, end: block.end })
}