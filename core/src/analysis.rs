use crate::{explorer::Visitor, parser::{Argument, Expression, ExpressionKind, Identifier, ImportSpecifier, LiteralExpressionKind, Statement, StatementKind, ValueHolder, VariableDescriptor}};

#[derive(Debug, Clone, Copy)]
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
    Definition,
    Variable,
    Function(Vec<Argument>),
    Class,
    Method,
    Constant,
    Import(String)
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub scope: ScopeId,
}

pub type SymbolId = usize;

pub struct Export {
    pub name: String,
    pub symbol: Symbol
}

pub struct SymbolIndex {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub exports: Vec<Export>
}

impl From<ScopeBuilder> for SymbolIndex {
    fn from(builder: ScopeBuilder) -> Self {
        Self {
            scopes: builder.scopes,
            symbols: builder.symbols,
            exports: builder.exports
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

pub struct ScopeBuilder {
    pub scopes: Vec<Scope>,
    pub current: ScopeId,
    pub symbols: Vec<Symbol>,
    pub exports: Vec<Export>
}

impl ScopeBuilder {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                parent: None,
                children: vec![],
                symbols: vec![],
                span: None
            }],
            current: ScopeId(0),
            symbols: vec![],
            exports: vec![]
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

impl Visitor for ScopeBuilder {
    fn visit_statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Function { name, arguments, .. } => {
                self.define(name.value.to_string(), SymbolKind::Function(arguments.clone()), Span { start: name.start, end: name.end });
            }
            StatementKind::Class { name, .. } => {
                self.define(name.value.to_string(), SymbolKind::Class, Span { start: name.start, end: name.end });
            }
            StatementKind::Import { specifiers, source } => {
                for specifier in specifiers {
                    if let ImportSpecifier { local: Expression { kind: ExpressionKind::Literal { value: ValueHolder::String(name), .. }, .. } } = specifier {
                        self.define(name.to_string(), SymbolKind::Import(source.value.to_string()), Span { start: specifier.local.start, end: specifier.local.end });
                    }
                }
            }
            StatementKind::Export { declaration } => {
                match &declaration.kind {
                    StatementKind::Function { name, arguments, .. } => {
                        let function_symbol = Symbol {
                            name: name.value.to_string(),
                            span: Span { start: name.start, end: name.end },
                            kind: SymbolKind::Function(arguments.clone()),
                            scope: self.current
                        };
                        self.exports.push(Export { name: name.value.to_string(), symbol: function_symbol });
                    }
                    StatementKind::Class { name, .. } => {
                        let class_symbol = Symbol {
                            name: name.value.to_string(),
                            span: Span { start: name.start, end: name.end },
                            kind: SymbolKind::Class,
                            scope: self.current
                        };
                        self.exports.push(Export { name: name.value.to_string(), symbol: class_symbol });
                    }
                    _ => todo!()
                }
            },
            StatementKind::VariableDefinition { descriptor, .. } => {
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
                    self.define(identifier.value.to_string(), SymbolKind::Definition, Span { start: identifier.start, end: identifier.end });
                }
            }
            StatementKind::For { variable, .. } => {
                self.define(variable.value.clone(), SymbolKind::Definition, Span { start: variable.start, end: variable.end });
            }
            _ => {}
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } => {
                self.define(name.to_string(), SymbolKind::Variable, Span { start: expression.start, end: expression.end });
            }
            _ => {}
        }
    }

    fn visit_argument(&mut self, argument: &Argument) {
        // TODO: fix span
        self.define(argument.name.value.to_string(), SymbolKind::Definition, Span { start: argument.name.start, end: argument.name.end });
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

// use crate::{explorer::{Visitor, visit_program}, parser::{Expression, ExpressionKind, Program, Statement, StatementKind}};

// pub enum SymbolType {
//     Variable,
//     Function,
//     Class,
//     Method,
//     Constant
// }

// struct SymbolCollector {
//     symbols: Vec<(String, SymbolType)>,
//     position: u32
// }

// impl SymbolCollector {
//     fn new(position: u32) -> Self {
//         Self {
//             symbols: Vec::new(),
//             position
//         }
//     }
// }

// impl Visitor for SymbolCollector {
//     fn visit_expression(&self, expression: &crate::parser::Expression) {

//     }
// }

// pub fn find_symbols_before(position: u32, program: &Program) -> Vec<(String, SymbolType)> {
//     let visitor = SymbolCollector::new(position);

//     visit_program(program, &visitor);

//     return visitor.symbols;
// }