use crate::{errors::{LanguageError, LanguageResult}, expect_array, expect_boolean, expect_comma_separated_group, expect_identifier, expect_number, expect_object, expect_string, expect_token, lexer::{Token, TokenKind}, parser::{ASTStatement, Expression, ExpressionKind, LiteralExpressionKind, Parser, ParserError, StatementKindWrapper, ValueHolder, expect_lambda}};

fn parse_prefix(parser: &mut Parser) -> LanguageResult<Expression> {
    let token = match parser.peek() {
        Some(token) => token.clone(),
        None => return Err(LanguageError::with_source(
            ParserError::UnexpectedEndOfInput,
            parser.tokens.last().map(|t| t.end).unwrap_or(0),
            parser.tokens.last().map(|t| t.end).unwrap_or(0),
        ))
    };

    if matches!(token.kind, TokenKind::Minus | TokenKind::Bang) {
        parser.advance();
        let left = parse_expression(parser, 0)?;
        let end = left.end;

        return Ok(ExpressionKind::Unary {
            left: Box::new(left),
            operator: token.kind
        }.into_expression(token.start, end))
    }

    if matches!(token.kind, TokenKind::OpeningParenthesis) {
        let checkpoint = parser.cursor;

        if let Ok(ASTStatement { kind, start, end }) = expect_lambda(parser) {
            return Ok(ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(Box::new(StatementKindWrapper::AST(kind))), value: ValueHolder::Void }.into_expression(start, end))
        }

        parser.cursor = checkpoint;

        parser.advance();

        let expression = parse_expression(parser, 0)?;

        expect_token!(parser, TokenKind::ClosingParenthesis);

        return Ok(expression)
    }

    let expression_kind: ExpressionKind = match parser.peek().map(|token| &token.kind) {
        Some(TokenKind::StringLiteral { .. }) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::String(expect_string!(parser).value) 
        },
        Some(TokenKind::NumericLiteral { .. }) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Number(expect_number!(parser).parse().unwrap()) 
        },
        Some(TokenKind::BooleanLiteral { .. }) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Bool(expect_boolean!(parser)) 
        },
        Some(TokenKind::Identifier { .. }) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Variable,
            value: ValueHolder::String(expect_identifier!(parser).value)
        },
        Some(TokenKind::OpeningSquareBracket) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Array(expect_array!(parser)),
            value: ValueHolder::Void
        },
        Some(TokenKind::OpeningBracket) => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Object(expect_object!(parser)),
            value: ValueHolder::Void
        },
        _ => parser.error()?
    };

    Ok(expression_kind.into_expression(token.start, parser.get_token_at(parser.cursor - 1).end))
}

fn make_expression_kind(operator: TokenKind, left: Box<Expression>, right: Box<Expression>) -> ExpressionKind {
    match operator {
        TokenKind::Assign | TokenKind::PlusEqual | TokenKind::MinusEqual | TokenKind::AsteriskEqual | TokenKind::SlashEqual | TokenKind::PercentEqual => ExpressionKind::Assignment { left, operator, right },
        TokenKind::And | TokenKind::Or => ExpressionKind::Logical { left, operator, right },
        TokenKind::EQ | TokenKind::NE => ExpressionKind::Equality { left, operator, right },
        TokenKind::GT | TokenKind::GTE | TokenKind::LT | TokenKind::LTE => ExpressionKind::Relational { left, operator, right },
        TokenKind::Plus | TokenKind::Minus => ExpressionKind::Binary { left, operator, right },
        TokenKind::Asterisk | TokenKind::Slash => ExpressionKind::Binary { left, operator, right },
        _ => unreachable!(),
    }
}

fn infix_binding_power(operator: &TokenKind) -> Option<(u8, u8)> {
    match operator {
        TokenKind::Assign | TokenKind::PlusEqual | TokenKind::MinusEqual | TokenKind::AsteriskEqual | TokenKind::SlashEqual | TokenKind::PercentEqual => Some((0, 1)),
        TokenKind::And | TokenKind::Or => Some((0, 1)),
        TokenKind::EQ | TokenKind::NE => Some((1, 2)),
        TokenKind::GT | TokenKind::GTE | TokenKind::LT | TokenKind::LTE => Some((3, 4)),
        TokenKind::Plus | TokenKind::Minus => Some((5, 6)),
        TokenKind::Asterisk | TokenKind::Slash => Some((7, 8)),
        _ => None,
    }
}

pub fn parse_expression(parser: &mut Parser, min_bp: u8) -> LanguageResult<Expression> {
    let mut lhs = parse_prefix(parser)?;

    let start = lhs.start;

    loop {
        if matches!(parser.peek(), Some(Token { kind: TokenKind::OpeningParenthesis, .. })) {
            let call_bp = 9;

            if call_bp < min_bp {
                break;
            }

            let arguments = expect_comma_separated_group!(parser, TokenKind::OpeningParenthesis, TokenKind::ClosingParenthesis, {
                parse_expression(parser, 0)?
            });

            lhs = ExpressionKind::Call {
                callee: Box::new(lhs),
                arguments,
            }.into_expression(start, parser.get_token_at(parser.cursor - 1).end);

            continue;
        }

        let separation_token = parser.peek().cloned();

        if matches!(separation_token, Some(Token { kind: TokenKind::Period | TokenKind::OpeningSquareBracket, .. })) {
            let bracket_notation = matches!(parser.peek(), Some(Token { kind: TokenKind::OpeningSquareBracket, .. }));

            let member_bp = 9;

            if member_bp < min_bp {
                break;
            }

            parser.advance();

            let token = match parser.peek().cloned() {
                Some(token) => token,
                None => {
                    if parser.strict {
                        return Err(LanguageError::with_source(
                            ParserError::UnexpectedEndOfInput,
                            lhs.start,
                            parser.tokens.last().map(|t| t.end).unwrap_or(lhs.start),
                        ))
                    }

                    let rhs = ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String("<lax>".to_string()) }.into_expression(separation_token.as_ref().unwrap().end, separation_token.as_ref().unwrap().end);
                    let start = lhs.start;
                    let end = rhs.end;
                    return Ok(ExpressionKind::Member { object: Box::new(lhs), property: Box::new(rhs) }.into_expression(start, end))
                }
            };

            let property = if bracket_notation {
                parse_expression(parser, 0)?
            } else {
                let name = match &token.kind {
                    TokenKind::Identifier { value } => value.clone(),
                    _ => {
                        if parser.strict {
                            return Err(LanguageError::with_source(
                                ParserError::UnexpectedToken("identifier after '.'".into()),
                                token.start,
                                token.end,
                            ))
                        }

                        let rhs = ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String("<lax>".to_string()) }.into_expression(separation_token.unwrap().end, token.start);
                        let start = lhs.start;
                        let end = rhs.end;
                        return Ok(ExpressionKind::Member { object: Box::new(lhs), property: Box::new(rhs) }.into_expression(start, end))
                    }
                };

                parser.advance();

                let start = if parser.strict {
                    token.start
                } else {
                    separation_token.unwrap().end
                };
                
                ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String(name) }.into_expression(start, token.end)
            };

            if bracket_notation {
                expect_token!(parser, TokenKind::ClosingSquareBracket);
            }

            lhs = ExpressionKind::Member {
                object: Box::new(lhs),
                property: Box::new(property)
            }.into_expression(token.start, token.end);

            continue;
        }

        let operator = match parser.peek() {
            Some(op) => op.kind.clone(),
            None => break,
        };

        let (l_bp, r_bp) = match infix_binding_power(&operator) {
            Some(bp) => bp,
            None => break,
        };

        if l_bp < min_bp {
            break;
        }

        parser.advance();

        let rhs = parse_expression(parser, r_bp)?;

        let end = rhs.end;

        lhs = make_expression_kind(
            operator,
            Box::new(lhs),
            Box::new(rhs)
        ).into_expression(start, end);
    }

    Ok(lhs)
}