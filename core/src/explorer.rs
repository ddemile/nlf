use crate::{analysis::{ScopeId, Span}, parser::{Argument, Block, Expression, ExpressionKind, LiteralExpressionKind, Program, Statement, StatementKind}};

pub trait Visitor {
    fn visit_expression(&mut self, _expression: &Expression) {}
    fn visit_statement(&mut self, _statement: &Statement) {}
    fn visit_argument(&mut self, _argument: &Argument) {}

    fn enter_scope(&mut self, _span: Span) -> ScopeId { ScopeId(0) }
    fn exit_scope(&mut self, _parent: ScopeId) {}
}

fn walk_block(visitor: &mut dyn Visitor, block: &Block) {
    for statement in block.statements.iter() {
        walk_statement(visitor, statement);
    }
}

pub fn walk_statement(visitor: &mut dyn Visitor, statement: &Statement) {
    visitor.visit_statement(statement);

    match &statement.kind {
        StatementKind::Function { block, arguments, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            
            for argument in arguments {
                visitor.visit_argument(argument);
            }

            walk_block(visitor, block);
            visitor.exit_scope(scope);
        }
        StatementKind::If { block, alternate, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            walk_block(visitor, block);
            visitor.exit_scope(scope);
            if let Some(alternate) = alternate {
                let scope = visitor.enter_scope(block.get_span());
                walk_statement(visitor, alternate);
                visitor.exit_scope(scope);
            }
        }
        StatementKind::Block(block) => {
            walk_block(visitor, block);
        }
        StatementKind::While { statements, .. } => {
            for statement in statements.iter() {
                walk_statement(visitor, statement);
            }
        }
        StatementKind::For { statements, .. } => {
            for statement in statements.iter() {
                walk_statement(visitor, statement);
            }
        }
        StatementKind::Class { methods, fields, ..} => {
            for method in methods.iter() {
                walk_statement(visitor, method);
            }

            for field in fields.iter() {
                walk_statement(visitor, field);
            }
        }
        StatementKind::Method { block, .. } => {
            let scope = visitor.enter_scope(block.get_span());
            walk_block(visitor, block);
            visitor.exit_scope(scope);
        }
        StatementKind::Export { declaration } => {
            walk_statement(visitor, declaration);
        }
        StatementKind::Return { expression } => {
            walk_expression(visitor, expression);
        }
        StatementKind::Expression { expression } => {
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
            if let LiteralExpressionKind::Function(block) = r#type {
                // TODO: This is a bit of a hack, but it works for now. We should probably refactor this to be more elegant.
                walk_statement(visitor, &block.clone().into_statement(0, 0));
            }
        }
        _ => {}
    }
}

pub fn visit_program(program: &Program, visitor: &mut dyn Visitor) {
    for statement in program.body.iter() {
        walk_statement(visitor, statement);
    }
}