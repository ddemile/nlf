
use crate::{analysis::{ScopeId, Span}, parser::{Block, Expression, ExpressionKind, Iterable, LiteralExpressionKind, Program, Statement, StatementKind, SyntaxTree}};

pub trait Visitor<A: SyntaxTree> {
    fn visit_expression(&mut self, _expression: &Expression) {}
    fn visit_statement(&mut self, _statement: &Statement<StatementKind<A>>) {}
    fn visit_argument(&mut self, _argument: &A::Argument) {}

    fn transform_expression(&mut self, expression: &Expression) -> Expression { expression.clone() }
    fn transform_statement(&mut self, statement: &Statement<StatementKind<A>>) -> Statement<StatementKind<A>> { statement.clone() }
    fn transform_argument(&mut self, argument: &A::Argument) -> A::Argument { argument.clone() }

    fn enter_scope(&mut self, _span: Span) -> ScopeId { ScopeId(0) }
    fn exit_scope(&mut self, _parent: ScopeId) {}
}

fn walk_block<A: SyntaxTree + 'static>(visitor: &mut dyn Visitor<A>, block: &Block<Statement<StatementKind<A>>>) -> Block<Statement<StatementKind<A>>> {
    Block {
        statements: block.statements.iter().map(|statement| walk_statement(visitor, statement)).collect(),
        start: block.start,
        end: block.end
    }
}

fn walk_statement<A: SyntaxTree + 'static>(visitor: &mut dyn Visitor<A>, statement: &Statement<StatementKind<A>>) -> Statement<StatementKind<A>> {
    visitor.visit_statement(statement);

    let statement = visitor.transform_statement(statement);

    let kind = match &statement.kind {
        StatementKind::Function { variable, block, arguments, return_type_ref, return_ty } => {
            let scope = visitor.enter_scope(block.get_span());
            
            let arguments = arguments.iter().map(|argument| {
                visitor.visit_argument(argument);
                visitor.transform_argument(argument).clone()
            }).collect();

            let block = walk_block(visitor, block);
            visitor.exit_scope(scope);

            StatementKind::Function { variable: variable.clone(), arguments, block, return_type_ref: return_type_ref.clone(), return_ty: return_ty.clone() }
        }
        StatementKind::If { condition, block, alternate } => {
            let scope = visitor.enter_scope(block.get_span());
            let block = walk_block(visitor, block);
            visitor.exit_scope(scope);
            if let Some(alternate) = alternate {
                let scope = visitor.enter_scope(block.get_span());
                let alternate = Box::new(walk_statement(visitor, &alternate));
                visitor.exit_scope(scope);
                StatementKind::If { condition: walk_expression(visitor, condition), block, alternate: Some(alternate) }
            } else {
                StatementKind::If { condition: walk_expression(visitor, condition), block, alternate: None }
            }
        }
        StatementKind::Block(block) => {
            StatementKind::Block(walk_block(visitor, block))
        }
        StatementKind::While { condition, statements } => {
            StatementKind::While { condition: condition.clone(), statements: statements.iter().map(|statement| walk_statement(visitor, statement)).collect() }
        }
        StatementKind::For { variable, statements, iterable } => {
            let iterable = match iterable {
                Iterable::Range(left, right) => {
                    let left = left.clone().map(|left| walk_expression(visitor, &left));
                    let right = right.clone().map(|right| walk_expression(visitor, &right));

                    Iterable::Range(left, right)
                }
                Iterable::Array(expression) => {
                    Iterable::Array(walk_expression(visitor, expression))
                }
            };

            let statements = statements.iter().map(|statement| walk_statement(visitor, statement)).collect();

            StatementKind::For { variable: variable.clone(), iterable, statements }
        }
        StatementKind::Class { variable, methods, fields, body_start, body_end } => {
            let scope = visitor.enter_scope(Span { start: *body_start, end: *body_end });
            // TODO: fix that
            let methods = methods.iter().map(|method| walk_statement(visitor, method)).collect();

            fields.iter().for_each(|field| {
                let Statement { kind: StatementKind::Field { visibility, name, value }, start, end } = field else {
                    unreachable!()
                };
                
                walk_statement(visitor, &Statement { kind: StatementKind::Field {
                    name: name.clone(),
                    visibility: visibility.clone(),
                    value: value.clone()
                }, start: *start, end: *end });
            });

            visitor.exit_scope(scope);

            StatementKind::Class { variable: variable.clone(), methods, fields: fields.to_vec(), body_start: *body_start, body_end: *body_end }
        }
        StatementKind::Method { name, arguments, block } => {
            let scope = visitor.enter_scope(block.get_span());
            // TODO: find where Method is instanciated
            // walk_block(visitor, block);
            visitor.exit_scope(scope);

            StatementKind::Method { name: name.clone(), arguments: arguments.clone(), block: block.clone() }
        }
        StatementKind::Export { declaration } => {
            StatementKind::Export { declaration: Box::new(walk_statement(visitor, declaration)) }
        }
        StatementKind::Return { expression } => {
            StatementKind::Return { expression: walk_expression(visitor, expression) }
        }
        StatementKind::Expression { expression } => {
            StatementKind::Expression { expression: walk_expression(visitor, expression) }
        }
        StatementKind::VariableDefinition { expression, descriptor, type_ref, ty } => {
            StatementKind::VariableDefinition { expression: walk_expression(visitor, expression), descriptor: descriptor.clone(), type_ref: type_ref.clone(), ty: ty.clone() }
        }
        StatementKind::Break => StatementKind::Break,
        StatementKind::Field { visibility, name, value } => {
            StatementKind::Field { visibility: visibility.clone(), name: name.clone(), value: value.clone() }
        }
        StatementKind::Import { specifiers, source } => {
            StatementKind::Import { specifiers: specifiers.clone(), source: source.clone() }
        }
    };

    kind.into_statement(statement.start, statement.end)
}

fn walk_expression<A: SyntaxTree + 'static>(visitor: &mut dyn Visitor<A>, expression: &Expression) -> Expression {
    visitor.visit_expression(expression);

    let expression = visitor.transform_expression(expression);

    let kind = match &expression.kind {
        ExpressionKind::Binary { left, operator, right } => {
            ExpressionKind::Binary { left: Box::new(walk_expression(visitor, left)), operator: operator.clone(), right: Box::new(walk_expression(visitor, right)) }
        }
        ExpressionKind::Unary { left, operator } => {
            ExpressionKind::Unary { left: Box::new(walk_expression(visitor, left)), operator: operator.clone() }
        }
        ExpressionKind::Equality { left, operator, right } => {
            ExpressionKind::Equality { left: Box::new(walk_expression(visitor, left)), operator: operator.clone(), right: Box::new(walk_expression(visitor, right)) }
        }
        ExpressionKind::Logical { left, operator, right, .. } => {
            ExpressionKind::Logical { left: Box::new(walk_expression(visitor, left)), operator: operator.clone(), right: Box::new(walk_expression(visitor, right)) }
        }
        ExpressionKind::Relational { left, operator, right } => {
            ExpressionKind::Relational { left: Box::new(walk_expression(visitor, left)), operator: operator.clone(), right: Box::new(walk_expression(visitor, right)) }
        }
        ExpressionKind::Assignment { left, operator, right } => {
            ExpressionKind::Assignment { left: Box::new(walk_expression(visitor, left)), operator: operator.clone(), right: Box::new(walk_expression(visitor, right)) }
        }
        ExpressionKind::Call { callee, arguments } => {
            let callee = walk_expression(visitor, callee);
            let arguments = arguments.iter().map(|argument| {
                walk_expression(visitor, argument)
            }).collect();

            ExpressionKind::Call { callee: Box::new(callee), arguments }
        }
        ExpressionKind::Member { object, property } => {
            ExpressionKind::Member { object: Box::new(walk_expression(visitor, object)), property: Box::new(walk_expression(visitor, property)) }
        }
        ExpressionKind::Literal { r#type, value } => {
            let r#type = match r#type {
                LiteralExpressionKind::Function(statement_kind_wrapper) => {
                    // TODO: This is a bit of a hack, but it works for now. We should probably refactor this to be more elegant.
                    let statement_kind = A::unwrap_statement_kind_wrapper(*statement_kind_wrapper.clone());

                    let Statement { kind: statement_kind, .. } = walk_statement(visitor, &statement_kind.into_statement(0, 0));
 
                    LiteralExpressionKind::Function(Box::new(A::wrap_statement_kind_wrapper(statement_kind)))
                }
                LiteralExpressionKind::Array(values) => {
                    let values = values.iter().map(|value| {
                        walk_expression(visitor, value)
                    }).collect();

                    LiteralExpressionKind::Array(values)
                }
                LiteralExpressionKind::Object(map) => {
                    let mut map = map.clone();
                    for (key, value) in map.clone() {
                        map.insert(key.clone(), walk_expression(visitor, &value)).unwrap();
                    }

                    LiteralExpressionKind::Object(map)
                }
                kind => kind.clone()
            };
            ExpressionKind::Literal { r#type, value: value.clone() }
        }
        kind => kind.clone()
    };

    Expression { kind, start: expression.start, end: expression.end }
}

pub fn visit_program<A: SyntaxTree + 'static>(program: &Program<Statement<StatementKind<A>>>, visitor: &mut dyn Visitor<A>) {
    for statement in program.body.iter() {
        walk_statement(visitor, statement);
    }
}

pub fn transform_program<A: SyntaxTree + 'static>(program: &Program<Statement<StatementKind<A>>>, visitor: &mut dyn Visitor<A>) -> Program<Statement<StatementKind<A>>> {
    let transformed_statements = program.body.iter().map(|statement| walk_statement(visitor, statement)).collect();

    Program { body: transformed_statements }
}