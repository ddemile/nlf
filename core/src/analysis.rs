use crate::{explorer::Visitor, parser::{Argument, Expression, ExpressionKind, ImportSpecifier, LiteralExpressionKind, Statement, StatementKind, ValueHolder}};

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

pub enum SymbolKind {
    Variable,
    Function,
    Class,
    Method,
    Constant
}

pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub scope: ScopeId,
}

pub type SymbolId = usize;

pub struct SymbolIndex {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
}

impl From<ScopeBuilder> for SymbolIndex {
    fn from(builder: ScopeBuilder) -> Self {
        Self {
            scopes: builder.scopes,
            symbols: builder.symbols
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
}

pub struct ScopeBuilder {
    pub scopes: Vec<Scope>,
    pub current: ScopeId,
    pub symbols: Vec<Symbol>
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
            symbols: vec![]
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
            StatementKind::Function { name, arguments: _, .. } => {
                self.define(name.to_string(), SymbolKind::Function, Span { start: statement.start, end: statement.end });
            }
            StatementKind::Class { name, .. } => {
                self.define(name.to_string(), SymbolKind::Class, Span { start: statement.start, end: statement.end });
            }
            StatementKind::Import { specifiers, .. } => {
                for specifier in specifiers {
                    if let ImportSpecifier { local: Expression { kind: ExpressionKind::Literal { value: ValueHolder::String(name), .. }, .. } } = specifier {
                        self.define(name.to_string(), SymbolKind::Variable, Span { start: statement.start, end: statement.end });
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Assignment { left, .. } => {
                if let ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } = &left.kind {
                    self.define(name.to_string(), SymbolKind::Variable, Span { start: expression.start, end: expression.end });
                }
            }
            _ => {}
        }
    }

    fn visit_argument(&mut self, argument: &Argument) {
        // TODO: fix span
        self.define(argument.name.to_string(), SymbolKind::Variable, Span { start: 0, end: 0 });
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