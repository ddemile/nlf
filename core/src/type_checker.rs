use std::{fs, rc::Rc, str::FromStr};

use crate::{analysis::ScopeBuilder, errors::LanguageResult, parser::{ASTBlock, ASTProgram, ASTStatement, Argument, Block, Expression, ExpressionKind, Identifier, LiteralExpressionKind, StatementKind, Type, TypeArena, TypeRef, TypedProgram, TypedStatement, TypedStatementKind, ValueHolder, VariableDescriptor}}; 

struct TypeChecker {
    pub program: ASTProgram,
    pub arena: TypeArena
}

impl TypeChecker {
    pub fn new(program: ASTProgram) -> Self {
        let arena = TypeArena::new();

        Self { program, arena }
    }
}

pub fn check_types(program: ASTProgram) -> LanguageResult<TypedProgram> {
    let mut checker = TypeChecker::new(program);
    
    check_body(checker.program.body.clone(), &mut checker).map(|statements| TypedProgram { body: statements })
}

fn check_body(statements: Rc<[ASTStatement]>, checker: &mut TypeChecker) -> LanguageResult<Rc<[TypedStatement]>> {
    statements.iter().map(|statement| {
        check_statement(statement.clone(), checker)
    }).collect()
}

fn check_statement(statement: ASTStatement, checker: &mut TypeChecker) -> LanguageResult<TypedStatement> {
    let kind: TypedStatementKind = match statement.kind {
        StatementKind::Break => StatementKind::Break,
        StatementKind::Block(block) => StatementKind::Block(check_block(block, checker)?),
        StatementKind::Function { variable, arguments, block, return_type_ref, return_ty: ty } => {
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
            StatementKind::Expression { expression }
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
        StatementKind::Method { name, arguments, block } => todo!(),
        StatementKind::Return { expression } => StatementKind::Return { expression },
        StatementKind::VariableDefinition { descriptor, expression, type_ref, .. } => check_definition(descriptor, expression, type_ref, checker)?,
        StatementKind::While { condition, statements } => {
            StatementKind::While { condition, statements: check_body(statements, checker)? }
        }
        _ => todo!()
    };

    Ok(TypedStatement {
        kind,
        start: statement.start,
        end: statement.end
    })
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

fn check_definition(descriptor: VariableDescriptor, expression: Expression, type_ref: Option<TypeRef>, _checker: &mut TypeChecker) -> LanguageResult<TypedStatementKind> {
    let ty = if let Some(type_ref) = &type_ref {
        resolve_type_ref(type_ref)?
    } else {
        resolve_expression_type(&expression)
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

fn resolve_expression_type(expression: &Expression) -> Type {
    match &expression.kind {
        ExpressionKind::Literal { r#type, value } => {
            match (r#type, value) {
                (LiteralExpressionKind::Literal, ValueHolder::String(_)) => Type::String,
                (LiteralExpressionKind::Literal, ValueHolder::Number(_)) => Type::Number,
                (LiteralExpressionKind::Literal, ValueHolder::Bool(_)) => Type::Bool,
                _ => Type::Unknown
            }    
        }
        _ => Type::Unknown
    }
}