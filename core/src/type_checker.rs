use std::{fs, path::PathBuf, rc::Rc, str::FromStr};


use nlf_shared::indexmap::IndexMap;

use crate::{analysis::{self, Scope, ScopeId, Span, Symbol, SymbolIndex, SymbolKind, TypedScopeBuilder, get_class_type}, errors::{LanguageError, LanguageResult}, explorer::{self, Visitor}, loader::{self}, parser::{ASTBlock, ASTProgram, ASTStatement, ASTStatementKind, ASTSyntaxTree, Argument, Block, Expression, ExpressionKind, FunctionType, Identifier, Iterable, LiteralExpressionKind, ParserError, Statement, StatementKind, StatementKindWrapper, Type, TypeArena, TypeRef, TypedProgram, TypedStatement, TypedStatementKind, TypedSyntaxTree, ValueHolder, VariableDescriptor}}; 

struct TypeChecker {
    pub program: ASTProgram,
    pub arena: TypeArena,
    pub symbol_index: Option<SymbolIndex>,
    pub type_collector: TypeCollector,
    pub hoisted_types: Vec<LocatedType>
}

impl TypeChecker {
    pub fn new(program: ASTProgram, type_collector: TypeCollector) -> Self {
        let arena = TypeArena::new();

        Self { program, arena, symbol_index: None, type_collector, hoisted_types: vec![] }
    }
}

pub struct CollectedType {
    pub name: String,
    pub span: Span,
    pub scope: ScopeId
}

pub struct TypeCollector {
    pub scopes: Vec<Scope>,
    pub current: ScopeId,
    pub types: Vec<CollectedType>,
    pub path: PathBuf
}

impl TypeCollector {
    pub fn new(path: PathBuf) -> Self {
        Self {
            scopes: vec![Scope {
                parent: None,
                children: vec![],
                symbols: vec![],
                span: None
            }],
            current: ScopeId(0),
            types: vec![],
            path
        }
    }

    fn define(&mut self, name: String, span: Span) {
        let id = self.types.len();

        self.types.push(CollectedType {
            name,
            span,
            scope: self.current
        });

        self.scopes[self.current.0]
            .symbols
            .push(id);
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

    pub fn find_definition(&self, name: &str, pos: usize) -> Option<&CollectedType> {
        let scope = self.scope_at_position(pos);

        let mut current = Some(scope);

        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];

            for &symbol_id in &scope.symbols {
                let ty = &self.types[symbol_id];
                
                if ty.name == name {
                    return Some(ty)
                }
            }

            current = scope.parent;
        }

        None
    }
}

impl Visitor<ASTSyntaxTree> for TypeCollector {
    fn transform_statement(&mut self, statement: &ASTStatement) -> ASTStatement {
        match &statement.kind {
            ASTStatementKind::Class { variable, .. } => {
                self.define(variable.value.clone(), Span { start: variable.start, end: variable.end });
            }
            ASTStatementKind::Import { specifiers, .. } => {
                for ident in specifiers {
                    self.define(ident.value.to_string(), Span { start: ident.start, end: ident.end });
                }
            }
            _ => {} 
        };

        statement.clone()
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

struct TypedTreeSyntaxTransformer {
    pub path: PathBuf,
    pub symbol_index: SymbolIndex,
    pub flat_statements: Vec<TypedStatement>
}

impl TypedTreeSyntaxTransformer {
    pub fn find_statement(&self, pos: usize) -> Option<TypedStatement> {
        let mut statement_option = None;

        for statement in self.flat_statements.iter() {
            if pos >= statement.start && pos <= statement.end {
                statement_option = Some(statement.clone())
            }
        }

        statement_option
    }

    fn find_variable_type(&self, name: &str, pos: usize) -> Option<Type> {
        let Some(Symbol { kind, span: definition_span, .. }) = self.symbol_index.find_definition(name, pos) else {
            return None
        };

        if let Some(definition) = self.find_statement(definition_span.start) {
            match definition.kind {
                StatementKind::VariableDefinition { ty: defintion_ty, .. } => {
                    return Some(defintion_ty)
                }
                StatementKind::Import { source, .. } => {
                    let module_source = loader::resolve_module(&source.value, Some(self.path.parent().unwrap().to_path_buf())).ok()?;

                    let contents = loader::read_module(&module_source).ok()?;

                    let symbol_index = analysis::get_symbol_index(module_source.path.into(), contents)?;

                    let source_symbol = symbol_index.find_export(name);

                    let Some(source_symbol) = source_symbol else {
                        return None
                    };
                    
                    match source_symbol.kind {
                        SymbolKind::Function(function_type) => return Some(Type::Function(Box::new(function_type))),
                        SymbolKind::Class(class_type) => return Some(Type::Class(Box::new(class_type))),
                        _ => return None
                    }
                }
                _ => {}
            }
        }

        // Tries to get the type of an argument
        if let SymbolKind::Definition(definition) = kind {
            return Some(definition.clone())
        }

        match kind {
            SymbolKind::Function(function_type) => {
                return Some(Type::Function(Box::new(function_type.clone())))
            }
            SymbolKind::Class(class_type) => {
                return Some(Type::Class(Box::new(class_type.clone())))
            }
            _ => {}
        }

        None
    }

    fn resolve_expression_type(&self, expression: &Expression) -> Option<Type> {
        match &expression.kind {
            ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } => {
                if let Some(variable_ty) = self.find_variable_type(&name, expression.start) {
                    return Some(variable_ty)
                }
            }
            ExpressionKind::Member { object, property } => {
                let ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(name) } = &object.kind else {
                    return None
                };

                let ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String(property) } = &property.kind else {
                    return None
                };

                let Some(Type::Instance { class }) = self.find_variable_type(name, object.start) else {
                    return None
                };

                if let Some(method) = class.methods.get(property) {
                    return Some(Type::Function(Box::new(method.clone())))
                }
            }
            ExpressionKind::Call { callee, .. } => {
                let Some(callee_type) = self.resolve_expression_type(&callee) else {
                    return None;
                };

                match callee_type {
                    Type::Function(box FunctionType { return_ty, .. }) => {
                        return Some(return_ty)
                    }
                    Type::Class(class_type) => {
                        return Some(Type::Instance { class: class_type })
                    }
                    _ => {}
                }
            }
            ExpressionKind::Literal { r#type: LiteralExpressionKind::Object(entries), .. } => {
                let map: IndexMap<String, Type> = entries.iter().filter_map(|(key, value)| {
                    let ty = self.resolve_expression_type(value).unwrap_or(resolve_expression_type(value).ok()?);

                    Some((key.to_string(), ty))
                }).collect();
                
                return Some(Type::Object(map))
            }
            _ => {}
        }
        
        None
    }
}

impl Visitor<TypedSyntaxTree> for TypedTreeSyntaxTransformer {
    fn transform_statement(&mut self, statement: &Statement<StatementKind<TypedSyntaxTree>>) -> Statement<StatementKind<TypedSyntaxTree>> {        
        let mut statement = statement.clone();
        
        if let StatementKind::VariableDefinition { ty, expression, .. } = &mut statement.kind {
            if matches!(ty, Type::Unknown) {
                if let Some(expression_type) = self.resolve_expression_type(expression) {
                    *ty = expression_type
                }
            }
        }

        self.flat_statements.push(statement.clone());

        statement
    }
}

pub fn check_types(program: ASTProgram, path: PathBuf) -> LanguageResult<TypedProgram> {
    let mut type_collector = TypeCollector::new(path.clone());

    explorer::visit_program(&program, &mut type_collector);

    let mut checker = TypeChecker::new(program, type_collector);
    
    let typed_program = check_body(checker.program.body.clone(), &mut checker).map(|statements| TypedProgram { body: statements })?;

    let mut visitor = TypedScopeBuilder::new();
    
    explorer::visit_program(&typed_program, &mut visitor);

    let symbol_index = SymbolIndex::from(visitor);

    let mut transformer = TypedTreeSyntaxTransformer { path, symbol_index, flat_statements: vec![] };

    let checked_program = explorer::transform_program(&typed_program, &mut transformer);

    Ok(checked_program)
}

pub struct LocatedType {
    pub ty: Type,
    pub span: Span
}

fn hoist_types(statements: &Rc<[ASTStatement]>, checker: &mut TypeChecker) -> LanguageResult<()> {
    fn hoist_class(statement: &ASTStatement, checker: &mut TypeChecker) -> LanguageResult<()> {
        let statement = check_statement(statement.clone(), checker)?;

        let StatementKind::Class { variable, methods, fields, .. } = statement.kind else {
            unreachable!()
        };

        let class_type = get_class_type(&variable, &methods, &fields);

        checker.hoisted_types.insert(0, LocatedType { ty: Type::Instance { class: Box::new(class_type)}, span: Span { start: variable.start, end: variable.end } });
    
        Ok(())
    }

    for statement in statements.iter() {
        match &statement.kind {
            StatementKind::Class { .. } => {
                hoist_class(statement, checker)?
            }
            StatementKind::Export { declaration } => {
                if let StatementKind::Class { .. } = &declaration.kind {
                    hoist_class(&declaration, checker)?
                }
            }
            StatementKind::Import { specifiers, source } => {
                let parent = checker.type_collector.path.parent().unwrap().to_path_buf();

                let module_source = loader::resolve_module(&source.value, Some(parent))?;

                let contents = loader::read_module(&module_source)?;

                let symbol_index = analysis::get_symbol_index(module_source.path.into(), contents).unwrap();

                for ident in specifiers {
                    let source_symbol = symbol_index.find_export(&ident.value);

                    let Some(source_symbol) = source_symbol else {
                        return Err(LanguageError::with_source(ParserError::InvalidType(format!("Type not found: {}", ident.value)), ident.start, ident.end))
                    };
                    
                    match source_symbol.kind {
                        SymbolKind::Class(class_type) => {
                            checker.hoisted_types.insert(0, LocatedType { ty: Type::Instance { class: Box::new(class_type) }, span: Span { start: ident.start, end: ident.end } });
                        },
                        _ => {}
                    }  
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn check_body(statements: Rc<[ASTStatement]>, checker: &mut TypeChecker) -> LanguageResult<Rc<[TypedStatement]>> {
    hoist_types(&statements, checker)?;

    statements.iter().map(|statement| {
        check_statement(statement.clone(), checker)
    }).collect()
}

fn check_statement(statement: ASTStatement, checker: &mut TypeChecker) -> LanguageResult<TypedStatement> {
    let ASTStatement { kind: statement_kind, start, end } = statement;

    let kind: TypedStatementKind = match statement_kind {
        StatementKind::Break => StatementKind::Break,
        StatementKind::Block(block) => StatementKind::Block(check_block(block, checker)?),
        StatementKind::Function { variable, arguments, block, return_type_ref, return_ty: _ty } => {
            let mut typed_arguments= vec![];
            for argument in arguments {
                typed_arguments.push(check_argument(argument, checker)?);
            }

            let return_ty = if let Some(type_ref) = &return_type_ref {
                resolve_type_ref(type_ref, checker)?
            } else {
                Type::Unknown
            };

            StatementKind::Function { variable, arguments: typed_arguments, block: check_block(block, checker)?, return_type_ref, return_ty }
        }
        StatementKind::Class { variable, methods, fields, body_start, body_end } => {
            StatementKind::Class { variable, methods: check_body(Rc::from(methods), checker)?.to_vec(), fields, body_start, body_end }
        }
        StatementKind::Export { declaration } => {
            StatementKind::Export { declaration: Box::new(check_statement(*declaration, checker)?) }
        }
        StatementKind::Expression { expression } => {
            StatementKind::Expression { expression: check_expression(expression, checker)? }
        }
        StatementKind::Field { visibility, name, value } => {
            StatementKind::Field { visibility, name, value }
        }
        StatementKind::For { variable, iterable, statements } => {
            let iterable = match iterable {
                Iterable::Range(left, right) => {
                    let left = left.map(|left| check_expression(left, checker)).transpose()?;
                    let right = right.map(|right| check_expression(right, checker)).transpose()?;

                    Iterable::Range(left, right)
                }
                Iterable::Array(expression) => {
                    Iterable::Array(check_expression(expression, checker)?)
                }
            };

            StatementKind::For { variable, iterable, statements: check_body(statements, checker)? }
        }
        StatementKind::If { condition, block, alternate } => {
            let alternate = if let Some(alt) = alternate {
                Some(Box::new(check_statement(*alt, checker)?))
            } else {
                None
            };

            StatementKind::If { condition, block: check_block(block, checker)?, alternate }
        }
        StatementKind::Import { specifiers, source } => StatementKind::Import { specifiers, source },
        StatementKind::Method { .. } => todo!(),
        StatementKind::Return { expression } => StatementKind::Return { expression },
        StatementKind::VariableDefinition { descriptor, expression, type_ref, .. } => check_definition(descriptor, expression, type_ref, checker)?,
        StatementKind::While { condition, statements } => {
            StatementKind::While { condition, statements: check_body(statements, checker)? }
        }
        _ => todo!()
    };

    Ok(TypedStatement {
        kind,
        start,
        end
    })
}

fn check_expression(expression: Expression, checker: &mut TypeChecker) -> LanguageResult<Expression> {
    let kind = match &expression.kind {
        ExpressionKind::Binary { left, operator, right } => {
            ExpressionKind::Binary { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone(), right: Box::new(check_expression(*right.clone(), checker)?) }
        }
        ExpressionKind::Unary { left, operator } => {
            ExpressionKind::Unary { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone() }
        }
        ExpressionKind::Equality { left, operator, right } => {
            ExpressionKind::Equality { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone(), right: Box::new(check_expression(*right.clone(), checker)?) }
        }
        ExpressionKind::Logical { left, operator, right, .. } => {
            ExpressionKind::Logical { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone(), right: Box::new(check_expression(*right.clone(), checker)?) }
        }
        ExpressionKind::Relational { left, operator, right } => {
            ExpressionKind::Relational { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone(), right: Box::new(check_expression(*right.clone(), checker)?) }
        }
        ExpressionKind::Assignment { left, operator, right } => {
            ExpressionKind::Assignment { left: Box::new(check_expression(*left.clone(), checker)?), operator: operator.clone(), right: Box::new(check_expression(*right.clone(), checker)?) }
        }
        ExpressionKind::Call { callee, arguments } => {
            let callee = check_expression(*callee.clone(), checker)?;
            let arguments= arguments.iter().map(|argument| {
                check_expression(argument.clone(), checker)
            }).collect::<LanguageResult<Vec<Expression>>>()?;


            ExpressionKind::Call { callee: Box::new(callee), arguments }
        }
        ExpressionKind::Member { object, property } => {
            ExpressionKind::Member { object: Box::new(check_expression(*object.clone(),checker)?), property: Box::new(check_expression(*property.clone(), checker)?) }
        }
        ExpressionKind::Literal { r#type, value } => {
            let r#type = match r#type {
                LiteralExpressionKind::Function(statement_kind_wrapper) => {
                    let mut statement_kind_wrapper = statement_kind_wrapper.clone();
                    if let box StatementKindWrapper::AST(statement) = statement_kind_wrapper {
                        let checked_statement = check_statement(statement.clone().into_statement(0, 0), checker)?;
                        statement_kind_wrapper = Box::new(StatementKindWrapper::Typed(checked_statement.kind))
                    }

                    LiteralExpressionKind::Function(statement_kind_wrapper)
                }
                LiteralExpressionKind::Array(values) => {
                    let values = values.iter().map(|value| {
                        check_expression(value.clone(), checker)
                    }).collect::<LanguageResult<Vec<Expression>>>()?;

                    LiteralExpressionKind::Array(values)
                }
                LiteralExpressionKind::Object(map) => {
                    let mut map = map.clone();
                    for (key, value) in map.clone() {
                        map.insert(key.clone(), check_expression(value, checker)?).unwrap();
                    }

                    LiteralExpressionKind::Object(map)
                }
                kind => kind.clone()
            };
            ExpressionKind::Literal { r#type, value: value.clone() }
        }
        kind => kind.clone()
    };

    Ok(Expression { kind, start: expression.start, end: expression.end })
}

fn check_block(block: ASTBlock, checker: &mut TypeChecker) -> LanguageResult<Block<TypedStatement>> {
    let checked_block = Block {
        statements: check_body(block.statements, checker)?,
        start: block.start,
        end: block.end
    };
    Ok(checked_block)
}

fn check_argument(argument: Argument<Identifier, ()>, checker: &mut TypeChecker) -> LanguageResult<Argument<Identifier, Type>> {
    let ty = if let Some(type_ref) = &argument.type_ref {
        resolve_type_ref(type_ref, checker)?
    } else {
        Type::Unknown
    };

    Ok(Argument {
        variable: argument.variable,
        type_ref: argument.type_ref,
        ty
    })
}

fn check_definition(descriptor: VariableDescriptor, expression: Expression, type_ref: Option<TypeRef>, checker: &mut TypeChecker) -> LanguageResult<TypedStatementKind> {
    let expression = check_expression(expression, checker)?;
    
    let ty = if let Some(type_ref) = &type_ref {
        resolve_type_ref(type_ref, checker)?
    } else {
        resolve_expression_type(&expression)?
    };
    
    Ok(TypedStatementKind::VariableDefinition { descriptor, expression, type_ref, ty })
}

fn resolve_type_ref(type_ref: &TypeRef, checker: &mut TypeChecker) -> LanguageResult<Type> {
    match type_ref {
        TypeRef::Named(ident) => {
            if let Ok(ty) = Type::from_str(&ident.value) {
                return Ok(ty)
            }

            if let Some(CollectedType { name, span, .. }) = checker.type_collector.find_definition(&ident.value, ident.start) {
                for located_type in checker.hoisted_types.iter() {
                    if located_type.span.start != span.start || located_type.span.end != span.end || *name != ident.value {
                        continue;
                    }

                    return Ok(located_type.ty.clone())
                }
            }

            return Err(LanguageError::with_source(ParserError::InvalidType(format!("Type not found: {}", ident.value)), ident.start, ident.end))
        }
        TypeRef::Array { type_ref, .. } => {
            return Ok(Type::Array(Box::new(resolve_type_ref(type_ref, checker)?)))
        }
        TypeRef::Object { entries, .. } => {
            let map: IndexMap<String, Type> = entries.iter().filter_map(|(key, type_ref)| {
                Some((key.to_string(), resolve_type_ref(type_ref, checker).ok()?))
            }).collect();

            return Ok(Type::Object(map))
        }
    }
}

fn resolve_expression_type(expression: &Expression) -> LanguageResult<Type> {
    match &expression.kind {
        ExpressionKind::Literal { r#type, value } => {
            match (r#type, value) {
                (LiteralExpressionKind::Literal, ValueHolder::String(_)) => Ok(Type::String),
                (LiteralExpressionKind::Literal, ValueHolder::Number(_)) => Ok(Type::Number),
                (LiteralExpressionKind::Literal, ValueHolder::Bool(_)) => Ok(Type::Bool),
                (LiteralExpressionKind::Function(box StatementKindWrapper::Typed(TypedStatementKind::Function { arguments, return_ty, .. })), _) => {
                    Ok(Type::Function(Box::new(FunctionType { arguments: arguments.clone(), return_ty: return_ty.clone() })))
                }
                _ => Ok(Type::Unknown)
            }    
        }
        _ => Ok(Type::Unknown)
    }
}