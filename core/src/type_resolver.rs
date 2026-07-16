use std::{fmt::Debug, sync::Arc};

use parking_lot::Mutex;

use crate::{analysis::{Scope, ScopeId, Span}, explorer::Visitor, parser::{ASTSyntaxTree, Type, TypeArena, TypeId}};

#[derive(Debug, Clone)]
pub struct TypeArenaWrapper<T: Debug = Type>(Arc<Mutex<TypeArena<T>>>);

impl<T: Debug + Clone> TypeArenaWrapper<T> {
    pub fn alloc(&self, ty: T) -> TypeId {
        self.0.lock().alloc(ty)
    }

    pub fn get(&self, id: TypeId) -> T {
        let arena = self.0.lock();
        arena.get(id).clone()
    }
}

#[derive(Debug, Clone)]
pub struct TypeIdent {
    pub name: String,
    pub span: Span,
}


#[derive(Debug, Clone)]
pub struct AllocatedTypeIdent {
    pub name: String,
    pub span: Span,
    pub id: TypeId
}

pub struct TypeCollector {
    pub arena: TypeArenaWrapper<TypeIdent>,
    pub types: Vec<AllocatedTypeIdent>,
    pub current: ScopeId,
    pub scopes: Vec<Scope>,
}

impl TypeCollector {
    fn define(&mut self, ty: TypeIdent) -> TypeId {
        let id = self.types.len();

        let type_id = self.arena.alloc(ty.clone());

        let type_ident = AllocatedTypeIdent {
            name: ty.name,
            span: ty.span,
            id: type_id 
        };

        self.types.push(type_ident);

        self.scopes[self.current.0]
            .symbols
            .push(id);

        type_id
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

    pub fn find_definition(&self, type_ident: TypeIdent) -> Option<&TypeId> {
        let scope = self.scope_at_position(type_ident.span.start);

        let mut current = Some(scope);

        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];

            for &symbol_id in &scope.symbols {
                let allocated_type_ident = &self.types[symbol_id];
                
                if allocated_type_ident.name == type_ident.name {
                    return Some(&allocated_type_ident.id)
                }
            }

            current = scope.parent;
        }

        None
    }
}

impl Visitor<ASTSyntaxTree> for TypeCollector {
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