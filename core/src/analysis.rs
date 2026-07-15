use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;
use serde::Serialize;

use crate::{explorer::{self, Visitor}, lexer, parser::{self, ASTStatement, ASTStatementKind, Argument, ClassType, DefaultType, Expression, ExpressionKind, FieldType, FunctionType, Identifier, Iterable, LiteralExpressionKind, Type, TypeArena, TypeId, TypedStatement, TypedStatementKind, TypedSyntaxTree, ValueHolder, VariableDescriptor}, type_checker};

#[derive(Debug, Clone, Copy, Serialize, Hash, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub usize);

pub struct Scope {
    pub parent: Option<ScopeId>,
    pub children: Vec<ScopeId>,
    pub symbols: Vec<SymbolId>,
    pub span: Option<Span>
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Definition(TypeId),
    Variable,
    Function(FunctionType),
    Class(ClassType),
    Method,
    Constant,
    Import(String),
    Property(usize)
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub scope: ScopeId
}

pub type SymbolId = usize;

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub symbol: Symbol
}

pub struct SymbolIndex {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub exports: Vec<Export>,
    pub arena: Arc<Mutex<TypeArena>>
}

impl From<TypedScopeBuilder> for SymbolIndex {
    fn from(builder: TypedScopeBuilder) -> Self {
        Self {
            scopes: builder.scopes,
            symbols: builder.symbols,
            exports: builder.exports,
            arena: builder.arena
        }
    }
}

impl SymbolIndex {
    pub fn visible_symbols(&self, scope: ScopeId) -> Vec<&Symbol> {
        let mut result = Vec::new();

        let mut current = Some(scope);

        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];

            for &symbol_id in &scope.symbols {
                result.push(&self.symbols[symbol_id]);
            }

            current = scope.parent;
        }

        result
    }

    pub fn scope_at_position(&self, pos: usize) -> ScopeId {
        let mut best = ScopeId(0); // global scope
        let mut best_size = usize::MAX;

        for (i, scope) in self.scopes.iter().enumerate() {
            let Some(span) = scope.span else {
                continue;
            };

            if pos >= span.start && pos <= span.end {
                let size = span.end - span.start;

                // Smaller span = deeper scope
                if size < best_size {
                    best = ScopeId(i);
                    best_size = size;
                }
            }
        }

        best
    }

    pub fn symbol_at(&self, pos: usize) -> Option<&Symbol> {
        let mut deepest_symbol: Option<&Symbol> = None;
        for symbol in self.symbols.iter() {
            if pos >= symbol.span.start && pos <= symbol.span.end {
                if let Some(Symbol { span, .. }) = deepest_symbol {
                    if symbol.span.start > span.start || symbol.span.end < span.end {
                        deepest_symbol = Some(symbol);
                    }
                } else {
                    deepest_symbol = Some(symbol);
                }
            }
        }
        deepest_symbol
    }

    pub fn find_definition(&self, name: &str, pos: usize) -> Option<&Symbol> {
        let scope = self.scope_at_position(pos);

        let mut current = Some(scope);

        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];

            for &symbol_id in &scope.symbols {
                let symbol = &self.symbols[symbol_id];
                
                if matches!(symbol.kind, SymbolKind::Variable) {
                    continue;
                }

                if symbol.name == name {
                    return Some(symbol)
                }
            }

            current = scope.parent;
        }

        None
    }

    pub fn find_export(&self, name: &str) -> Option<Symbol> {
        for export in self.exports.iter() {
            if export.name == name {
                return Some(export.symbol.clone())
            }
        }

        None
    }
}

pub fn get_symbol_index(path: PathBuf, contents: String) -> Option<SymbolIndex> {
    let tokens = match lexer::lex(contents) {
        Ok(tokens) => tokens,
        Err(_) => return None
    };

    let program = match parser::lax_parse(tokens) {
        Ok(program) => program,
        Err(_) => return None
    };

    let typed_program = match type_checker::check_types(program, path) {
        Ok(program) => program,
        Err(_) => return None
    };

    let index = {
        let mut arena = TypeArena::new();
        arena.register_defaults();

        let mut scope_builder = TypedScopeBuilder::new(Arc::new(Mutex::new(arena)));

        explorer::visit_program(&typed_program, &mut scope_builder);

        SymbolIndex::from(scope_builder)
    };

    Some(index)
}

pub struct TypedScopeBuilder {
    pub scopes: Vec<Scope>,
    pub current: ScopeId,
    pub symbols: Vec<Symbol>,
    pub exports: Vec<Export>,
    pub arena: Arc<Mutex<TypeArena>>
}

impl TypedScopeBuilder {
    pub fn new(arena: Arc<Mutex<TypeArena>>) -> Self {
        Self {
            scopes: vec![Scope {
                parent: None,
                children: vec![],
                symbols: vec![],
                span: None
            }],
            current: ScopeId(0),
            symbols: vec![],
            exports: vec![],
            arena
        }
    }

    fn define(&mut self, name: String, kind: SymbolKind, span: Span) -> SymbolId {
        let id = self.symbols.len();

        self.symbols.push(Symbol {
            name,
            span,
            kind,
            scope: self.current,
        });

        self.scopes[self.current.0]
            .symbols
            .push(id);

        id
    }
}

impl Visitor<TypedSyntaxTree> for TypedScopeBuilder {
    fn visit_statement(&mut self, statement: &TypedStatement) {
        match &statement.kind {
            TypedStatementKind::Function { variable: name, arguments, return_ty, .. } => {
                self.define(name.value.to_string(), SymbolKind::Function(FunctionType { arguments: arguments.clone(), return_ty: return_ty.clone() }), Span { start: name.start, end: name.end });
            }
            TypedStatementKind::Class { variable: name, methods, fields, .. } => {
                self.define(name.value.to_string(), SymbolKind::Class(get_class_type(name, methods, fields, self.arena.clone())), Span { start: name.start, end: name.end });
            },
            TypedStatementKind::Import { specifiers, source } => {
                for specifier in specifiers {
                    self.define(specifier.value.to_string(), SymbolKind::Import(source.value.to_string()), Span { start: specifier.start, end: specifier.end });
                }
            }
            TypedStatementKind::Export { declaration } => {
                match &declaration.kind {
                    TypedStatementKind::Function { variable: name, arguments, return_ty, .. } => {
                        let function_symbol = Symbol {
                            name: name.value.to_string(),
                            span: Span { start: name.start, end: name.end },
                            kind: SymbolKind::Function(FunctionType { arguments: arguments.clone(), return_ty: return_ty.clone() }),
                            scope: self.current
                        };
                        self.exports.push(Export { name: name.value.to_string(), symbol: function_symbol });
                    }
                    TypedStatementKind::Class { variable: name, methods, fields, .. } => {
                        let class_type = get_class_type(name, methods, fields, self.arena.clone());

                        let class_symbol = Symbol {
                            name: name.value.to_string(),
                            span: Span { start: name.start, end: name.end },
                            kind: SymbolKind::Class(class_type),
                            scope: self.current
                        };
                        self.exports.push(Export { name: name.value.to_string(), symbol: class_symbol });
                    }
                    _ => todo!()
                }
            },
            TypedStatementKind::VariableDefinition { descriptor, ty, .. } => {
                fn get_identifiers(descriptor: &VariableDescriptor) -> Vec<Identifier> {
                    match descriptor {
                        VariableDescriptor::Identifier(identifier) => vec![identifier.clone()],
                        VariableDescriptor::Object(descriptors) => {
                            let mut identifiers = vec![];
                            for descriptor in descriptors {
                                let mut inner_identifiers = get_identifiers(descriptor);
                                identifiers.append(&mut inner_identifiers);
                            }
                            identifiers
                        }
                    }
                }

                for identifier in get_identifiers(descriptor) {
                    self.define(identifier.value.to_string(), SymbolKind::Definition(ty.clone()), Span { start: identifier.start, end: identifier.end });
                }
            }
            TypedStatementKind::For { variable, iterable, .. } => {
                match iterable {
                    Iterable::Range(_, _) => {
                        let number_ty = self.arena.lock().get_default(DefaultType::Number);
                        self.define(variable.value.clone(), SymbolKind::Definition(number_ty), Span { start: variable.start, end: variable.end });
                    }
                    Iterable::Array(_) => {
                        let unknown_ty = self.arena.lock().get_default(DefaultType::Number);
                        // TODO: implement corrrect variable type
                        self.define(variable.value.clone(), SymbolKind::Definition(unknown_ty), Span { start: variable.start, end: variable.end });
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } => {
                self.define(name.to_string(), SymbolKind::Variable, Span { start: expression.start, end: expression.end });
            }
            ExpressionKind::Member { object, property } => {
                let ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, .. } = object.kind else {
                    return
                };

                let ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String(value) } = &property.kind else {
                    return
                };

                self.define(value.to_string(), SymbolKind::Property(object.start), Span { start: property.start, end: property.end });
            }
            _ => {}
        }
    }

    fn visit_argument(&mut self, argument: &Argument<Identifier, TypeId>) {
        // TODO: fix span
        self.define(argument.variable.value.to_string(), SymbolKind::Definition(argument.ty.clone()), Span { start: argument.variable.start, end: argument.variable.end });
    }

    fn enter_scope(&mut self, span: Span) -> ScopeId {
        let parent = self.current;

        let id = ScopeId(self.scopes.len());

        self.scopes.push(Scope {
            parent: Some(self.current),
            children: vec![],
            symbols: vec![],
            span: Some(span)
        });

        self.scopes[self.current.0]
            .children
            .push(id);

        self.current = id;

        parent
    }

    fn exit_scope(&mut self, parent: ScopeId) {
        self.current = parent;
    }
}

pub fn get_class_type(name: &Identifier, methods: &Vec<TypedStatement>, fields: &Vec<ASTStatement>, arena: Arc<Mutex<TypeArena>>) -> ClassType {
    let method_types = methods.iter().map(|method| {
        let TypedStatementKind::Function { variable, arguments, return_ty, .. } = &method.kind else {
            unreachable!()
        };

        (
            variable.value.clone(),
            FunctionType {
                arguments: arguments.clone(),
                return_ty: return_ty.clone()
            }
        )
    }).collect();

    let field_types = fields.iter().map(|field| {
        let ASTStatementKind::Field { name, visibility, .. } = &field.kind else {
            unreachable!()
        };
        
        FieldType {
            name: name.clone(),
            visibility: visibility.clone(),
            ty: arena.lock().get_default(DefaultType::Unknown)
        }
    }).collect();

    ClassType { name: name.value.to_string(), methods: method_types, fields: field_types }
}

pub struct CompletionCandidate {
    pub completion_span: Span,
    pub object_span: Span
}

#[derive(Default)]
pub struct CompletionResolver {
    pub candidates: Vec<CompletionCandidate>
}

impl Visitor<TypedSyntaxTree> for CompletionResolver {
    fn visit_expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Member { object, property } => {
                if let ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String(_value) } = &property.kind {
                    self.candidates.push(CompletionCandidate { completion_span: Span { start: property.start, end: property.end }, object_span: Span { start: object.start, end: object.end } });
                }
            }
            _ => {}
        }
    }
}