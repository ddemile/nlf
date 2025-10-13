use std::collections::HashMap;

use serde::Serialize;

use crate::lexer::{KeywordKind, Token};

#[derive(Serialize, Debug, Clone)]
pub struct Function {
    pub arguments: Vec<VariableRef>,
    pub statements: Vec<Statement>,
    pub scope_position: usize,
}

#[derive(Serialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ObjectRef {
    pub object_id: usize,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum ValueHolder {
    Int(i32),
    String(String),
    Float(f64),
    Bool(bool),
    Fn(Function),
    Object(ObjectRef),
    Void,
}

impl PartialEq for ValueHolder {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueHolder::Int(a), ValueHolder::Int(b)) => a == b,
            (ValueHolder::String(a), ValueHolder::String(b)) => a == b,
            (ValueHolder::Float(a), ValueHolder::Float(b)) => a == b,
            (ValueHolder::Bool(a), ValueHolder::Bool(b)) => a == b,
            (ValueHolder::Void, ValueHolder::Void) => true,
            _ => false, // different variants are never equal
        }
    }
}

impl PartialOrd for ValueHolder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (ValueHolder::Int(a), ValueHolder::Int(b)) => a.partial_cmp(b),
            (ValueHolder::Float(a), ValueHolder::Float(b)) => a.partial_cmp(b),
            (ValueHolder::Int(a), ValueHolder::Float(b)) => a.partial_cmp(&(*b as i32)),
            (ValueHolder::Float(a), ValueHolder::Int(b)) => a.partial_cmp(&(*b as f64)),
            (ValueHolder::String(a), ValueHolder::String(b)) => a.partial_cmp(b),
            (ValueHolder::Bool(a), ValueHolder::Bool(b)) => a.partial_cmp(b),
            (ValueHolder::Void, ValueHolder::Void) => Some(std::cmp::Ordering::Equal),
            _ => None, // different variants cannot be compared
        }
    }
}

impl Into<bool> for ValueHolder {
    fn into(self) -> bool {
        match self {
            ValueHolder::Int(a) => a > 0,
            ValueHolder::Float(a) => a > 0.0,
            ValueHolder::String(a) => a.len() > 0,
            ValueHolder::Bool(a) => a,
            _ => false, // different variants cannot be compared
        }
    }
}

impl Into<i32> for ValueHolder {
    fn into(self) -> i32 {
        match self {
            ValueHolder::Int(a) => a,
            ValueHolder::Float(a) => a.floor() as i32,
            _ => panic!("Couldn't ValueHolder convert into i32"),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")] // "type" field will contain the variant name
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind")] // "kind" field will contain the variant name
pub enum Statement {
    Expression {
        expression: Expression,
    },
    If {
        condition: Expression,
        block: Block,
        alternate: Option<Box<Statement>>,
    },
    For {
        variable: Expression,
        left: Option<Expression>,
        right: Option<Expression>,
        statements: Vec<Statement>,
    },
    ForIR {
        variable: VariableRef,
        left: Option<Expression>,
        right: Option<Expression>,
        statements: Vec<Statement>,
    },
    Function {
        name: String,
        arguments: Vec<Argument>,
        statements: Vec<Statement>,
    },
    FunctionIR {
        var_ref: VariableRef,
        arguments: Vec<VariableRef>,
        statements: Vec<Statement>,
    },
    Return {
        expression: Expression,
    },
    Break,
    Block(Block),
}

#[derive(Serialize, Debug, Clone)]
pub enum LiteralExpressionKind {
    Literal,
    Variable,
    Object(HashMap<String, Expression>),
}

#[derive(Serialize, Debug, Clone)]
pub struct VariableRef {
    pub slot: usize,
    pub depth: usize,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "#type")] // "type" field will contain the variant name
pub enum Expression {
    Literal {
        r#type: LiteralExpressionKind,
        value: ValueHolder,
    },
    Variable(VariableRef),
    Member {
        object: Box<Expression>,
        property: Box<Expression>,
    },
    Binary {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    Relational {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    Logical {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    Equality {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Assignment {
        left: Box<Expression>,
        right: Box<Expression>,
        is_definition: bool,
    },
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")] // "type" field will contain the variant name
pub struct Program {
    pub body: Vec<Statement>,
}

fn parse_internal(mut tokens: Vec<Token>) -> (Vec<Statement>, usize) {
    let mut cursor = 0;
    let mut statements: Vec<Statement> = vec![];

    while cursor < tokens.len() {
        let token: &Token = tokens.get(cursor).unwrap();

        match token {
            Token::Keyword(KeywordKind::If) => {
                cursor += 1;

                statements.push(parse_if(&mut cursor, &mut tokens))
            }
            Token::Keyword(KeywordKind::For) => {
                cursor += 1;

                let Some(Token::Identifier { value: _ }) = tokens.get(cursor) else {
                    panic!("Expected identifier")
                };

                let loop_variable: Expression = literal_expression(&mut cursor, &mut tokens);

                let Some(Token::Keyword(KeywordKind::In)) = tokens.get(cursor) else {
                    panic!("Expected 'in' keyword")
                };

                cursor += 1;

                let left = matches!(
                    tokens.get(cursor),
                    Some(Token::NumericLiteral { .. })
                        | Some(Token::Identifier { .. })
                        | Some(Token::OpeningParenthesis)
                )
                .then(|| literal_expression(&mut cursor, &mut tokens));

                if !matches!(tokens.get(cursor), Some(Token::Range)) {
                    panic!("Expected range (..) token");
                }

                cursor += 1;

                let right = matches!(
                    tokens.get(cursor),
                    Some(Token::NumericLiteral { .. })
                        | Some(Token::Identifier { .. })
                        | Some(Token::OpeningParenthesis)
                )
                .then(|| literal_expression(&mut cursor, &mut tokens));

                let inner_statements = block(&mut cursor, &mut tokens);

                match (&left, &right) {
                    (None, None) => panic!("Both sides of a range expression cannot be None"),
                    _ => {}
                }

                statements.push(Statement::For {
                    variable: loop_variable,
                    left,
                    right,
                    statements: inner_statements,
                });
            }
            Token::Keyword(KeywordKind::Fn) => {
                cursor += 1;

                let Some(Token::Identifier { value }) = tokens.get(cursor) else {
                    panic!("Expected identifier");
                };

                let value = value.clone();

                if matches!(tokens.get(cursor + 1), Some(Token::OpeningParenthesis)) {
                    cursor += 2; // move to the first character after (

                    let mut args: Vec<Argument> = vec![];

                    while let Some(token) = tokens.get(cursor) {
                        match token {
                            Token::ClosingParenthesis => {
                                // End of argument list
                                break;
                            }
                            _ => {
                                // Parse the argument
                                let expr = literal_expression(&mut cursor, &mut tokens);

                                let Expression::Literal {
                                    r#type: LiteralExpressionKind::Variable,
                                    value: ValueHolder::String(name),
                                } = expr
                                else {
                                    panic!();
                                };

                                args.push(Argument { name });

                                // After parsing argument, check if next token is a comma
                                match tokens.get(cursor) {
                                    Some(Token::Comma) => cursor += 1, // skip comma, continue loop
                                    Some(Token::ClosingParenthesis) => break, // done
                                    _ => panic!("Expected ',' or ')' after argument"),
                                }
                            }
                        }
                    }

                    if !matches!(tokens.get(cursor), Some(Token::ClosingParenthesis)) {
                        panic!("Expected ')'");
                    }

                    cursor += 1; // move past ')'

                    let inner_statements = block(&mut cursor, &mut tokens);

                    statements.push(Statement::Function {
                        name: value.to_string(),
                        arguments: args,
                        statements: inner_statements,
                    });
                }
            }
            Token::Keyword(KeywordKind::Return) => {
                cursor += 1;
                statements.push(Statement::Return {
                    expression: equality_expression(&mut cursor, &mut tokens),
                });
            }
            Token::Keyword(KeywordKind::Break) => {
                cursor += 1;
                statements.push(Statement::Break);
            }
            Token::ClosingBracket => break,
            _ => {
                statements.push(Statement::Expression {
                    expression: expression(&mut cursor, &mut tokens),
                });
            }
        }
    }

    return (statements, cursor);
}

pub fn parse(tokens: Vec<Token>) -> Program {
    return Program {
        body: parse_internal(tokens).0,
    };
}

fn parse_if(cursor: &mut usize, tokens: &mut Vec<Token>) -> Statement {
    let condition = equality_expression(cursor, tokens);

    let inner_statements = block(cursor, tokens);

    let mut alternate: Option<Box<Statement>> = None;

    if let Some(Token::Keyword(KeywordKind::Else)) = tokens.get(*cursor) {
        *cursor += 1;
        if let Some(Token::Keyword(KeywordKind::If)) = tokens.get(*cursor) {
            *cursor += 1;
            alternate = Some(Box::new(parse_if(cursor, tokens)));
        } else {
            alternate = Some(Box::new(Statement::Block(Block {
                statements: block(cursor, tokens),
            })));
        }
    }

    Statement::If {
        condition,
        block: Block {
            statements: inner_statements,
        },
        alternate,
    }
}

fn block(cursor: &mut usize, tokens: &mut Vec<Token>) -> Vec<Statement> {
    if !matches!(tokens.get(*cursor), Some(Token::OpeningBracket)) {
        panic!("Expected {{")
    }
    *cursor += 1;
    let (inner_statements, inner_cursor) = parse_internal(tokens[*cursor..].to_vec());
    *cursor += inner_cursor;

    if !matches!(tokens.get(*cursor), Some(Token::ClosingBracket)) {
        panic!("Expected }}")
    }

    *cursor += 1;

    inner_statements
}

fn expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    return assignment_expression(cursor, tokens);
}

fn assignment_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let is_definition = if let Some(Token::Keyword(KeywordKind::Let)) = tokens.get(*cursor) {
        *cursor += 1;
        true
    } else {
        false
    };

    let left = equality_expression(cursor, tokens);

    if matches!(left, Expression::Literal { .. }) || (!is_definition && matches!(left, Expression::Member { .. })) {
        if let Some(Token::Assign) = tokens.get(*cursor) {
            *cursor += 1;
            return Expression::Assignment {
                left: Box::new(left),
                right: Box::new(equality_expression(cursor, tokens)),
                is_definition,
            };
        }
    }

    return left;
}

fn equality_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let mut left: Expression = logical_expression(cursor, tokens);
    while matches!(tokens.get(*cursor), Some(Token::EQ | Token::NE)) {
        let operator = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        left = Expression::Equality {
            left: Box::new(left),
            operator,
            right: Box::new(logical_expression(cursor, tokens)),
        };
    }

    return left;
}

fn logical_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let mut left: Expression = relational_expression(cursor, tokens);
    while matches!(tokens.get(*cursor), Some(Token::And | Token::Or)) {
        let operator = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        left = Expression::Logical {
            left: Box::new(left),
            operator,
            right: Box::new(relational_expression(cursor, tokens)),
        };
    }

    return left;
}

fn relational_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let mut left = term_expression(cursor, tokens);
    while matches!(
        tokens.get(*cursor),
        Some(Token::GT | Token::GTE | Token::LT | Token::LTE)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        left = Expression::Relational {
            left: Box::new(left),
            operator,
            right: Box::new(term_expression(cursor, tokens)),
        };
    }

    return left;
}

fn term_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let mut left = factor_expression(cursor, tokens);
    while matches!(tokens.get(*cursor), Some(Token::Plus | Token::Minus)) {
        let operator = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(factor_expression(cursor, tokens)),
        };
    }

    return left;
}

fn factor_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let mut left = call_expression(cursor, tokens);
    while matches!(tokens.get(*cursor), Some(Token::Asterisk | Token::Slash)) {
        let operator = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(call_expression(cursor, tokens)),
        };
    }

    return left;
}

fn call_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    if let Some(Token::Identifier { value }) = tokens.get(*cursor) {
        if matches!(tokens.get(*cursor + 1), Some(Token::OpeningParenthesis)) {
            let name = value.clone();
            *cursor += 1; // move to the '('

            *cursor += 1; // move to the first token inside parentheses
            let mut args: Vec<Expression> = vec![];

            while let Some(token) = tokens.get(*cursor) {
                match token {
                    Token::ClosingParenthesis => {
                        // End of argument list
                        break;
                    }
                    _ => {
                        // Parse the argument
                        let expr = equality_expression(cursor, tokens);
                        args.push(expr);

                        // After parsing argument, check if next token is a comma
                        match tokens.get(*cursor) {
                            Some(Token::Comma) => *cursor += 1, // skip comma, continue loop
                            Some(Token::ClosingParenthesis) => break, // done
                            _ => panic!("Expected ',' or ')' after argument"),
                        }
                    }
                }
            }

            if !matches!(tokens.get(*cursor), Some(Token::ClosingParenthesis)) {
                panic!("Expected ')'");
            }

            *cursor += 1; // move past ')'

            return Expression::Call {
                callee: Box::new(Expression::Literal {
                    r#type: LiteralExpressionKind::Variable,
                    value: ValueHolder::String(name),
                }),
                arguments: args,
            };
        }
    }

    return literal_expression(cursor, tokens);
}

fn literal_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> Expression {
    let token = tokens.get(*cursor).unwrap();

    *cursor += 1;

    let json = serde_json::to_string_pretty(&token).unwrap();

    match token {
        Token::NumericLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Float(*value),
        },
        Token::BooleanLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Bool(*value),
        },
        Token::StringLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::String(value.clone()),
        },
        Token::Identifier { value } => {
            let expr = Expression::Literal {
                r#type: LiteralExpressionKind::Variable,
                value: ValueHolder::String(value.clone()),
            };

            member(cursor, tokens, expr, true)
        }
        Token::OpeningParenthesis => {
            let expr = expression(cursor, tokens);

            tokens.get(*cursor).map_or_else(
                || panic!(") expected"),
                |token| matches!(token, Token::ClosingParenthesis),
            );

            *cursor += 1;

            return expr;
        }
        Token::OpeningBracket => {
            let mut entries: HashMap<String, Expression> = HashMap::new();
            while !matches!(tokens.get(*cursor), Some(Token::ClosingBracket)) {
                let key = literal_expression(cursor, tokens);

                let Expression::Literal {
                    r#type: LiteralExpressionKind::Literal,
                    value: ValueHolder::String(key),
                } = key
                else {
                    todo!();
                };

                let Some(Token::Colon) = tokens.get(*cursor) else {
                    todo!();
                };

                *cursor += 1;

                let value = literal_expression(cursor, tokens);

                entries.insert(key, value);

                match tokens.get(*cursor) {
                    Some(Token::Comma) => {
                        *cursor += 1;
                        continue;
                    }
                    Some(Token::ClosingBracket) => {
                        break;
                    }
                    _ => todo!(),
                }
            }

            *cursor += 1;

            return Expression::Literal {
                r#type: LiteralExpressionKind::Object(entries),
                value: ValueHolder::Void,
            };
        }
        _ => panic!("Unexpected token {json}"),
    }
}

fn member(cursor: &mut usize, tokens: &mut Vec<Token>, mut expr: Expression, match_call: bool) -> Expression {
    loop {
        if matches!(tokens.get(*cursor), Some(Token::Period)) {
            *cursor += 1;
            let Some(Token::Identifier { value }) = tokens.get(*cursor).cloned() else {
                panic!()
            };

            *cursor += 1;

            expr = Expression::Member {
                object: Box::new(expr),
                property: Box::new(Expression::Literal {
                    r#type: LiteralExpressionKind::Variable,
                    value: ValueHolder::String(value.to_string()),
                }),
            };

            // Check if call
            if match_call && matches!(tokens.get(*cursor), Some(Token::OpeningParenthesis)) {
                *cursor += 1; // move to the first token inside parentheses
                let mut args: Vec<Expression> = vec![];

                while let Some(token) = tokens.get(*cursor) {
                    match token {
                        Token::ClosingParenthesis => {
                            // End of argument list
                            break;
                        }
                        _ => {
                            // Parse the argument
                            let expr = equality_expression(cursor, tokens);
                            args.push(expr);

                            // After parsing argument, check if next token is a comma
                            match tokens.get(*cursor) {
                                Some(Token::Comma) => *cursor += 1, // skip comma, continue loop
                                Some(Token::ClosingParenthesis) => break, // done
                                _ => panic!("Expected ',' or ')' after argument"),
                            }
                        }
                    }
                }

                if !matches!(tokens.get(*cursor), Some(Token::ClosingParenthesis)) {
                    panic!("Expected ')'");
                }

                *cursor += 1; // move past ')'

                expr = Expression::Call {
                    callee: Box::new(expr),
                    arguments: args,
                };
            }
        } else {
            break;
        }
    }

    return expr;
}
