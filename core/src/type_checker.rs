use std::{fs, path::PathBuf, rc::Rc, str::FromStr};


use crate::{analysis::{self, Symbol, SymbolIndex, SymbolKind, TypedScopeBuilder}, errors::LanguageResult, explorer::{self, Visitor}, loader::{self, ModuleSource}, parser::{ASTBlock, ASTProgram, ASTStatement, Argument, Block, Expression, ExpressionKind, FunctionType, Identifier, LiteralExpressionKind, Statement, StatementKind, StatementKindWrapper, Type, TypeArena, TypeRef, TypedProgram, TypedStatement, TypedStatementKind, TypedSyntaxTree, ValueHolder, VariableDescriptor}}; 

struct TypeChecker {
    pub program: ASTProgram,
    pub arena: TypeArena,
    pub symbol_index: Option<SymbolIndex>
}

impl TypeChecker {
    pub fn new(program: ASTProgram) -> Self {
        let arena = TypeArena::new();

        Self { program, arena, symbol_index: None }
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
    let mut checker = TypeChecker::new(program);
    
    let typed_program = check_body(checker.program.body.clone(), &mut checker).map(|statements| TypedProgram { body: statements })?;

    let mut visitor = TypedScopeBuilder::new();
    
    explorer::visit_program(&typed_program, &mut visitor);

    let symbol_index = SymbolIndex::from(visitor);

    let mut transformer = TypedTreeSyntaxTransformer { path, symbol_index, flat_statements: vec![] };

    let checked_program = explorer::transform_program(&typed_program, &mut transformer);

    Ok(checked_program)
}

fn check_body(statements: Rc<[ASTStatement]>, checker: &mut TypeChecker) -> LanguageResult<Rc<[TypedStatement]>> {
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
                typed_arguments.push(check_argument(argument)?);
            }

            let return_ty = if let Some(type_ref) = &return_type_ref {
                resolve_type_ref(type_ref)?
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
        StatementKind::For { variable, left, right, statements } => {
            StatementKind::For { variable, left, right, statements: check_body(statements, checker)? }
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

fn check_argument(argument: Argument<Identifier, ()>) -> LanguageResult<Argument<Identifier, Type>> {
    let ty = if let Some(type_ref) = &argument.type_ref {
        resolve_type_ref(type_ref)?
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
        resolve_type_ref(type_ref)?
    } else {
        resolve_expression_type(&expression)?
    };
    
    Ok(TypedStatementKind::VariableDefinition { descriptor, expression, type_ref, ty })
}

fn resolve_type_ref(type_ref: &TypeRef) -> LanguageResult<Type> {
    Ok(match type_ref {
        TypeRef::Named(ident) => {
            Type::from_str(&ident.value)?
        }
    })
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