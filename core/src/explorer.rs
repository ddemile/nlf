use std::{fmt::format, fs};

use crate::{analysis::{ScopeId, Span}, parser::{Argument, Expression, ExpressionKind, Identifier, LiteralExpressionKind, Statement, StatementKind, StatementKindWrapper, Type, TypedBlock, TypedProgram, TypedStatement, TypedStatementKind}};

pub trait Visitor {
    fn visit_expression(&mut self, _expression: &Expression) {}
    fn visit_statement(&mut self, _statement: &TypedStatement) {}
    fn visit_argument(&mut self, _argument: &Argument<Identifier, Type>) {}

    fn enter_scope(&mut self, _span: Span) -> ScopeId { ScopeId(0) }
    fn exit_scope(&mut self, _parent: ScopeId) {}
}

fn walk_block(visitor: &mut dyn Visitor, block: &TypedBlock) {
    for statement in block.statements.iter() {
        walk_statement(visitor, statement);
    }
}

pub fn walk_statement(visitor: &mut dyn Visitor, statement: &TypedStatement) {
    visitor.visit_statement(statement);

    match &statement.kind {
        TypedStatementKind::Function { block, arguments, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            
            for argument in arguments {
                visitor.visit_argument(argument);
            }

            walk_block(visitor, block);
            visitor.exit_scope(scope);
        }
        TypedStatementKind::If { block, alternate, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            walk_block(visitor, block);
            visitor.exit_scope(scope);
            if let Some(alternate) = alternate {
                let scope = visitor.enter_scope(block.get_span());
                walk_statement(visitor, alternate);
                visitor.exit_scope(scope);
            }
        }
        TypedStatementKind::Block(block) => {
            walk_block(visitor, block);
        }
        TypedStatementKind::While { statements, .. } => {
            for statement in statements.iter() {
                walk_statement(visitor, statement);
            }
        }
        TypedStatementKind::For { statements, left, right, .. } => {
            if let Some(left) = left {
                walk_expression(visitor, left);
            }
            
            if let Some(right) = right {
                walk_expression(visitor, right);
            }

            for statement in statements.iter() {
                walk_statement(visitor, statement);
            }
        }
        TypedStatementKind::Class { methods, fields, body_start, body_end, .. } => {
            let scope = visitor.enter_scope(Span { start: *body_start, end: *body_end });
            // TODO: fix that
            for method in methods.iter() {
                walk_statement(visitor, method);
            }

            for field in fields.iter() {
                let Statement { kind: StatementKind::Field { visibility, name, value }, start, end } = field else {
                    unreachable!()
                };
                
                walk_statement(visitor, &Statement { kind: TypedStatementKind::Field {
                    name: name.clone(),
                    visibility: visibility.clone(),
                    value: value.clone()
                }, start: *start, end: *end });
            }

            visitor.exit_scope(scope);
        }
        TypedStatementKind::Method { block, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            // TODO: find where Method is instanciated
            // walk_block(visitor, block);
            visitor.exit_scope(scope);
        }
        TypedStatementKind::Export { declaration } => {
            walk_statement(visitor, declaration);
        }
        TypedStatementKind::Return { expression } => {
            walk_expression(visitor, expression);
        }
        TypedStatementKind::Expression { expression } => {
            walk_expression(visitor, expression);
        }
        TypedStatementKind::VariableDefinition { expression, .. } => {
            walk_expression(visitor, expression);
        }
        _ => {}
    }
}

fn walk_expression(visitor: &mut dyn Visitor, expression: &Expression) {
    visitor.visit_expression(expression);

    match &expression.kind {
        ExpressionKind::Binary { left, right, .. } => {
            walk_expression(visitor, left);
            walk_expression(visitor, right);
        }
        ExpressionKind::Unary { left, .. } => {
            walk_expression(visitor, left);
        }
        ExpressionKind::Equality { left, right, .. } => {
            walk_expression(visitor, left);
            walk_expression(visitor, right);
        }
        ExpressionKind::Logical { left, right, .. } => {
            walk_expression(visitor, left);
            walk_expression(visitor, right);
        }
        ExpressionKind::Relational { left, right, .. } => {
            walk_expression(visitor, left);
            walk_expression(visitor, right);
        }
        ExpressionKind::Assignment { left, right, .. } => {
            walk_expression(visitor, left);
            walk_expression(visitor, right);
        }
        ExpressionKind::Call { callee, arguments } => {
            walk_expression(visitor, callee);
            for argument in arguments.iter() {
                walk_expression(visitor, argument);
            }
        }
        ExpressionKind::Member { object, property } => {
            walk_expression(visitor, object);
            walk_expression(visitor, property);
        }
        ExpressionKind::Literal { r#type, .. } => {
            match r#type {
                LiteralExpressionKind::Function(block) => {
                    // TODO: This is a bit of a hack, but it works for now. We should probably refactor this to be more elegant.
                    let box StatementKindWrapper::AST(statement_kind) = block else {
                        unreachable!()
                    };
                    // walk_statement(visitor, &TypedStatement {
                    //     kind: &statement_kind.clone(),
                    //     start: 0,
                    //     end: 0
                    // });
                }
                LiteralExpressionKind::Array(values) => {
                    for value in values {
                        walk_expression(visitor, value);
                    }
                }
                LiteralExpressionKind::Object(map) => {
                    for value in map.values() {
                        walk_expression(visitor, value);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

pub fn visit_program(program: &TypedProgram, visitor: &mut dyn Visitor) {
    for statement in program.body.iter() {
        walk_statement(visitor, statement);
    }
}