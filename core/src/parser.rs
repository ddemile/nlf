
use crate::{errors::{LanguageError, LanguageResult}, lexer::{KeywordKind, Token, TokenKind}, parser::pratt::parse_expression};

pub mod pratt;
pub mod types;

pub use types::*;

pub struct Parser {
    pub cursor: usize,
    pub tokens: Vec<Token>
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            cursor: 0,
            tokens
        }
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn error<T>(&self) -> LanguageResult<T> {
        let token = self.peek();

        Err(match token {
            Some(Token { start, end, .. }) => {
                LanguageError::with_source(ParserError::UnexpectedToken(format!(
                    "unexpected token at index {}",
                    self.cursor
                )), *start, *end)
            }
            _ => {
                LanguageError::with_source(
                    ParserError::UnexpectedEndOfInput,
                    self.get_token_at(self.cursor - 1).end - 1,
                    self.get_token_at(self.cursor - 1).end
                )
            }
        })
    }

    pub fn consume(
        &mut self,
        predicate: impl Fn(&Token) -> bool,
    ) -> LanguageResult<Token> {
        match self.peek().cloned() {
            Some(token) if predicate(&token) => {
                self.advance();
                Ok(token)
            },
            _ => self.error()
        }
    }

    #[inline(always)]
    pub fn advance(&mut self) {
        self.cursor += 1;
    }

    fn get_token_at(&self, index: usize) -> Token {
        self.tokens.get(index).unwrap().clone()
    }
}

#[macro_export]
macro_rules! match_token {
    ($parser:expr, $kind:pat) => {
        $parser.consume(|t| matches!(t.kind, $kind))
    };
}

#[macro_export]
macro_rules! expect_token {
    ($parser:expr, $kind:pat) => {
        $parser.consume(|t| matches!(t.kind, $kind))?
    };
}

#[macro_export]
macro_rules! expect_keyword {
    ($parser:expr, $keyword:pat) => {
        expect_token!($parser, TokenKind::Keyword($keyword))
    };
}

#[macro_export]
macro_rules! expect_string {
    ($parser:expr) => {{
        let Token { kind: TokenKind::StringLiteral { value }, start, end } = crate::expect_token!($parser, TokenKind::StringLiteral { .. }) else {
            unreachable!()
        };
        crate::parser::types::StringLiteral { value, start, end }
    }};
}

#[macro_export]
macro_rules! expect_number {
    ($parser:expr) => {{
        let Token { kind: TokenKind::NumericLiteral { value }, .. } = crate::expect_token!($parser, TokenKind::NumericLiteral { .. }) else {
            unreachable!()
        };
        value
    }};
}

#[macro_export]
macro_rules! expect_boolean {
    ($parser:expr) => {{
        let Token { kind: TokenKind::BooleanLiteral { value }, .. } = crate::expect_token!($parser, TokenKind::BooleanLiteral { .. }) else {
            unreachable!()
        };
        value
    }};
}

#[macro_export]
macro_rules! expect_identifier {
    ($parser:expr) => {{
        let Token { kind: TokenKind::Identifier { value }, start, end } = crate::expect_token!($parser, TokenKind::Identifier { .. }) else {
            unreachable!()
        };
        crate::parser::types::Identifier { value, start, end }
    }};
}

#[macro_export]
macro_rules! expect_comma_separated_group {
    ($parser:expr, $start_token:pat, $end_token:pat, $matcher:block) => {{
        crate::expect_token!($parser, $start_token);

        let mut specifiers: Vec<_> = vec![];
        let mut should_end = false;

        loop {
            if let Some(Token { kind: $end_token, .. }) = $parser.peek().cloned() {
                $parser.consume(|t| matches!(t.kind, $end_token))?;
                break;
            } else if should_end {
                return $parser.error();
            }
            specifiers.push($matcher);
            if let Some(Token { kind: TokenKind::Comma, .. }) = $parser.peek().cloned() {
                $parser.consume(|t| matches!(t.kind, TokenKind::Comma))?;
                should_end = false;
            } else {
                should_end = true
            }
        }

        specifiers
    }};
}

#[macro_export]
macro_rules! expect_destructuring_pattern {
    ($parser:expr) => {{
        crate::expect_comma_separated_group!($parser, TokenKind::OpeningBracket, TokenKind::ClosingBracket, {
            crate::expect_token!($parser, TokenKind::Identifier { .. })
        })
    }};
}

#[macro_export]
macro_rules! expect_arguments_definition {
    ($parser:expr) => {{
        crate::expect_comma_separated_group!($parser, TokenKind::OpeningParenthesis, TokenKind::ClosingParenthesis, {
            parse_expression($parser, 0)?
        })
    }};
}

#[macro_export]
macro_rules! expect_array {
    ($parser:expr) => {{
        crate::expect_comma_separated_group!($parser, TokenKind::OpeningSquareBracket, TokenKind::ClosingSquareBracket, {
            parse_expression($parser, 0)?
        })
    }};
}

#[macro_export]
macro_rules! expect_object {
    ($parser:expr) => {{
        let mut map: indexmap::IndexMap<String, Expression> = indexmap::IndexMap::new();
        
        crate::expect_comma_separated_group!($parser, TokenKind::OpeningBracket, TokenKind::ClosingBracket, {
            let key = crate::expect_string!($parser);
            crate::expect_token!($parser, TokenKind::Colon);
            let value = parse_expression($parser, 0)?;
            map.insert(key.value, value);
            ()
        });

        map
    }};
}

pub fn expect_block(parser: &mut Parser) -> LanguageResult<Block> {
    let Token { start, .. } = expect_token!(parser, TokenKind::OpeningBracket);

    let mut statements: Vec<Statement> = vec![];

    if let Some(Token { kind: TokenKind::ClosingBracket, end, .. }) = parser.peek().cloned() {
        parser.consume(|t| matches!(t.kind, TokenKind::ClosingBracket))?;
        return Ok(Block { statements: statements.into(), start, end });
    }

    while let Some(statement) = match_statement(parser)? {
        statements.push(statement);

        if let Some(Token { kind: TokenKind::ClosingBracket, .. }) = parser.peek().cloned() {
            break;
        }
    }

    expect_token!(parser, TokenKind::ClosingBracket);

    Ok(Block {
        statements: statements.into(),
        start,
        end: parser.get_token_at(parser.cursor - 1).end
    })
}

fn expect_lambda(parser: &mut Parser) -> LanguageResult<Statement> {
    let start_cursor = parser.cursor;
    let arguments = expect_arguments_definition!(parser);
    let start = parser.get_token_at(start_cursor).start;

    expect_token!(parser, TokenKind::Arrow);

    let block = expect_block(parser)?;

    let end = block.end;

    Ok(StatementKind::Function {
        name: Identifier { value: format!("<lambda:{start}-{end}>"), start: 0, end: 0 },
        arguments: arguments.iter().map(|argument| {
            Argument {
                name: match argument {
                    Expression { kind: ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(value) }, start, end } => {
                        Identifier { value: value.to_string(), start: *start, end: *end }
                    },
                    _ => unreachable!()
                }
            }
        }).collect(),
        block
    }.into_statement(start, end))
}

pub fn parse(tokens: Vec<Token>) -> LanguageResult<Program> {
    let mut parser = Parser::new(tokens);

    let mut statements = vec![];

    while let Some(statement) = match_statement(&mut parser)? {
        statements.push(statement);
    }

    Ok(Program { body: statements.into() })
}

fn match_statement(parser: &mut Parser) -> LanguageResult<Option<Statement>> {
    let start = parser.cursor;

    match parser.peek().map(|token| &token.kind) {
        Some(TokenKind::Keyword(KeywordKind::Import)) => Ok(Some(match_import(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::Fn)) => Ok(Some(match_function(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::If)) => Ok(Some(match_if(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::Let)) => Ok(Some(match_let(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::Export)) => Ok(Some(match_export(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::For)) => Ok(Some(match_for(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::While)) => Ok(Some(match_while(parser)?)),
        Some(TokenKind::Keyword(KeywordKind::Break)) => {
            let Token { start, end, .. } = expect_keyword!(parser, KeywordKind::Break);

            Ok(Some(StatementKind::Break.into_statement(start, end)))
        },
        Some(TokenKind::Keyword(KeywordKind::Return)) => {
            let Token { start, .. } = expect_keyword!(parser, KeywordKind::Return);
            let expression = match_expression(parser)?;

            let end = expression.end;

            Ok(Some(StatementKind::Return { expression }.into_statement(start, end)))
        },
        Some(TokenKind::Keyword(KeywordKind::Class)) => Ok(Some(match_class(parser)?)),
        _ => {
            if parser.peek().is_none() {
                return Ok(None);
            }

            let expression = match_expression(parser)?;
            Ok(Some(StatementKind::Expression { expression }.into_statement(parser.get_token_at(start).start, parser.get_token_at(parser.cursor - 1).end)))
        }
    }
}

fn match_import(parser: &mut Parser) -> LanguageResult<Statement> {
    let start = parser.cursor;

    expect_keyword!(parser, KeywordKind::Import);

    let specifiers = expect_destructuring_pattern!(parser).iter().map(|specifier| {
        let Token { kind: TokenKind::Identifier { value }, start, end } = specifier else {
            unreachable!()
        };
        ImportSpecifier {
            local: Expression { kind: ExpressionKind::Literal { r#type: crate::parser::LiteralExpressionKind::Literal, value: ValueHolder::String(value.into()) }, start: *start, end: *end }
        }
    }).collect();

    expect_keyword!(parser, KeywordKind::From);
    let source = expect_string!(parser);

    let Token { start, end, .. } = parser.get_token_at(start);

    Ok(Statement { kind: StatementKind::Import { specifiers, source }, start, end })
}

fn match_method(parser: &mut Parser) -> LanguageResult<Statement> {
    let name = expect_identifier!(parser);
    let start = parser.get_token_at(parser.cursor - 1).start;

    let arguments = expect_arguments_definition!(parser);
    let block = expect_block(parser)?;

    let end = block.end;

    Ok(StatementKind::Function {
        name,
        arguments: arguments.iter().map(|argument| {
            Argument {
                name: match argument {
                    Expression { kind: ExpressionKind::Literal { r#type: LiteralExpressionKind::Variable, value: ValueHolder::String(value) }, start, end } => {
                        Identifier { value: value.to_string(), start: *start, end: *end }
                    },
                    _ => unreachable!()
                }
            }
        }).collect(),
        block
    }.into_statement(start, end))
}

fn match_function(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Fn);

    let mut method = match_method(parser)?;
    method.start = start;

    Ok(method)
}

fn match_if(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::If);
    let condition = match_expression(parser)?;
    let block = expect_block(parser)?;

    let mut alternate: Option<Box<Statement>> = None;

    if let Ok(_) = match_token!(parser, TokenKind::Keyword(KeywordKind::Else)) {
        if matches!(parser.peek(), Some(Token { kind: TokenKind::Keyword(KeywordKind::If), .. })) {
            alternate = Some(Box::new(match_if(parser)?))
        } else {
            let block = expect_block(parser)?;
            alternate = Some(Box::new(StatementKind::Block(block.clone()).into_statement(block.start, block.end)));
        }
    }

    Ok(StatementKind::If {
        condition,
        block,
        alternate
    }.into_statement(start, parser.get_token_at(parser.cursor - 1).end))
}

fn match_let(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Let);

    if let Expression { kind: ExpressionKind::Assignment { left, operator, right, .. }, end, .. } = match_expression(parser)? {
        Ok(StatementKind::Expression {
            expression: ExpressionKind::Assignment { left, operator, right, is_definition: true }.into_expression(start, end)
        }.into_statement(start, end))
    } else {
        let Token { start, end, .. } = parser.get_token_at(parser.cursor - 1);
        
        Err(LanguageError::with_source(ParserError::UnexpectedToken("'identifier'".into()), start, end))
    }
}

fn match_export(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Export);

    let Some(statement) = match_statement(parser)? else {
        return Err(LanguageError::from(ParserError::UnexpectedEndOfInput))
    };

    let end = statement.end;

    Ok(StatementKind::Export {
        declaration: Box::new(statement)
    }.into_statement(start, end))
}

fn match_for(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::For);
    let Token { kind: TokenKind::Identifier { value: loop_variable }, start: loop_variable_start, end: loop_variable_end } = expect_token!(parser, TokenKind::Identifier { .. }) else {
        unreachable!()
    };
    expect_keyword!(parser, KeywordKind::In);

    let left = match parser.peek() {
        Some(Token { kind: TokenKind::NumericLiteral { .. } | TokenKind::Identifier { .. } | TokenKind::OpeningParenthesis, .. }) => Some(match_expression(parser)?),
        _ => None
    };
    
    let Token { start: range_start, end: range_end, .. } = expect_token!(parser, TokenKind::Range);

    let right = match parser.peek() {
        Some(Token { kind: TokenKind::NumericLiteral { .. } | TokenKind::Identifier { .. } | TokenKind::OpeningParenthesis, .. }) => Some(match_expression(parser)?),
        _ => None
    };

    let block = expect_block(parser)?;

    match (&left, &right) {
        (None, None) => return Err(LanguageError::with_source(ParserError::InvalidUsage("Both sides of a range expression cannot be None".into()), range_start, range_end)),
        _ => ()
    }

    Ok(StatementKind::For {
        variable: ExpressionKind::Literal { r#type: LiteralExpressionKind::Literal, value: ValueHolder::String(loop_variable) }.into_expression(loop_variable_start, loop_variable_end),
        left,
        right,
        statements: block.statements
    }.into_statement(start, block.end))
}

fn match_while(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::While);
    let condition = match_expression(parser)?;
    let block = expect_block(parser)?;

    Ok(StatementKind::While {
        condition,
        statements: block.statements
    }.into_statement(start, block.end))
}

fn match_class(parser: &mut Parser) -> LanguageResult<Statement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Class);
    let name = expect_identifier!(parser);
    
    let mut methods: Vec<_> = vec![];
    let mut fields: Vec<_> = vec![];

    expect_token!(parser, TokenKind::OpeningBracket);
    
    loop {
        if let Ok(_) = match_token!(parser, TokenKind::ClosingBracket) {
            break;
        }

        if let Ok(_) = match_token!(parser, TokenKind::Keyword(KeywordKind::Fn)) {
            parser.cursor -= 1;
            methods.push(match_function(parser)?);
        } else if let Ok(_) = match_token!(parser, TokenKind::Identifier { .. }) {
            parser.cursor -= 1;
            methods.push(match_method(parser)?);
        } else {
            let (start, visibility) = match expect_token!(parser, TokenKind::Keyword(KeywordKind::Public | KeywordKind::Protected | KeywordKind::Private)) {
                Token { kind: TokenKind::Keyword(keyword), start, .. } => (start, Visibility::from(&TokenKind::Keyword(keyword))),
                _ => unreachable!()
            };

            let Identifier { value: field_name, .. } = expect_identifier!(parser);

            expect_token!(parser, TokenKind::Assign);
            let expression = parse_expression(parser, 0)?;
            let end = expression.end;

            fields.push(StatementKind::Field {
                name: field_name,
                value: expression,
                visibility
            }.into_statement(start, end));
        }
    }

    Ok(StatementKind::Class {
        name,
        methods,
        fields
    }.into_statement(start, parser.get_token_at(parser.cursor - 1).end))
}

fn match_expression(parser: &mut Parser) -> LanguageResult<Expression> {
    parse_expression(parser, 0)
}