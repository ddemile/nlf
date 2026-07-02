use std::rc::Rc;

use crate::{analysis::Span, errors::{LanguageError, LanguageResult}, lexer::{KeywordKind, Token, TokenKind}, parser::pratt::parse_expression};

pub mod pratt;
pub mod types;

use nlf_shared::indexmap::IndexMap;
pub use types::*;

pub struct Parser {
    pub cursor: usize,
    pub tokens: Vec<Token>,
    pub strict: bool
}

impl Parser {
    pub fn new(tokens: Vec<Token>, strict: bool) -> Self {
        Self {
            cursor: 0,
            tokens,
            strict
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
            let identifier = expect_identifier!($parser);

            let mut type_ref = None;

            if matches!($parser.peek(), Some(Token { kind: TokenKind::Colon, .. })) {
                $parser.advance();

                type_ref = Some(match_type_ref($parser)?);
            }
            
            crate::parser::types::Argument {
                variable: identifier,
                type_ref,
                ty: ()
            }
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
        let mut map: nlf_shared::indexmap::IndexMap<String, Expression> = nlf_shared::indexmap::IndexMap::new();
        
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

pub fn expect_block(parser: &mut Parser) -> LanguageResult<ASTBlock> {
    let Token { end: start, .. } = expect_token!(parser, TokenKind::OpeningBracket);

    let mut statements: Vec<ASTStatement> = vec![];

    if let Some(Token { kind: TokenKind::ClosingBracket, start: end, .. }) = parser.peek().cloned() {
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

fn expect_lambda(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let start_cursor = parser.cursor;

    let arguments = expect_arguments_definition!(parser);

    let mut return_type_ref = None;

    if matches!(parser.peek(), Some(Token { kind: TokenKind::Colon, .. })) {
        parser.advance();

        let type_ref = match_type_ref(parser)?;

        return_type_ref = Some(type_ref);
    }

    let start = parser.get_token_at(start_cursor).start;

    expect_token!(parser, TokenKind::Arrow);

    let block = if matches!(parser.peek(), Some(Token { kind: TokenKind::OpeningBracket, .. })) {
        expect_block(parser)?
    } else {
        let expression = match_expression(parser)?;

        let start = expression.start;
        let end = expression.end;

        Block { statements: Rc::new([StatementKind::Return { expression }.into_statement(start, end)]), start, end }
    };

    let end = block.end;

    Ok(ASTStatementKind::Function {
        variable: Identifier { value: format!("<lambda:{start}-{end}>"), start: 0, end: 0 },
        arguments,
        block,
        return_type_ref: return_type_ref,
        return_ty: ()
    }.into_statement(start, end))
}

pub fn parse(tokens: Vec<Token>) -> LanguageResult<Program<ASTStatement>> {
    let mut parser = Parser::new(tokens, true);

    let mut statements = vec![];

    while let Some(statement) = match_statement(&mut parser)? {
        statements.push(statement);
    }

    Ok(Program { body: statements.into() })
}

pub fn lax_parse(tokens: Vec<Token>) -> LanguageResult<Program<ASTStatement>> {
    let mut parser = Parser::new(tokens, false);

    let mut statements = vec![];

    while let Some(statement) = match_statement(&mut parser)? {
        statements.push(statement);
    }

    Ok(Program { body: statements.into() })
}

fn match_statement(parser: &mut Parser) -> LanguageResult<Option<ASTStatement>> {
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

            Ok(Some(ASTStatementKind::Break.into_statement(start, end)))
        },
        Some(TokenKind::Keyword(KeywordKind::Return)) => {
            let Token { start, .. } = expect_keyword!(parser, KeywordKind::Return);
            let expression = match_expression(parser)?;

            let end = expression.end;

            Ok(Some(ASTStatementKind::Return { expression }.into_statement(start, end)))
        },
        Some(TokenKind::Keyword(KeywordKind::Class)) => Ok(Some(match_class(parser)?)),
        _ => {
            if parser.peek().is_none() {
                return Ok(None);
            }

            let expression = match_expression(parser)?;
            Ok(Some(ASTStatementKind::Expression { expression }.into_statement(parser.get_token_at(start).start, parser.get_token_at(parser.cursor - 1).end)))
        }
    }
}

fn match_import(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Import);

    let specifiers = expect_destructuring_pattern!(parser).iter().map(|specifier| {
        let Token { kind: TokenKind::Identifier { value }, start, end } = specifier else {
            unreachable!()
        };
        Identifier { value: value.to_string(), start: *start, end: *end }
    }).collect();

    expect_keyword!(parser, KeywordKind::From);
    let source = expect_string!(parser);

    let Token { end, .. } = parser.get_token_at(parser.cursor - 1);

    Ok(ASTStatement { kind: ASTStatementKind::Import { specifiers, source }, start, end })
}

fn match_method(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let name = expect_identifier!(parser);
    let start = name.start;

    let arguments = expect_arguments_definition!(parser);

    let mut return_type_ref = None;

    if matches!(parser.peek(), Some(Token { kind: TokenKind::Colon, .. })) {
        parser.advance();

        return_type_ref = Some(match_type_ref(parser)?);
    }

    let block = expect_block(parser)?;

    let end = block.end;

    Ok(ASTStatementKind::Function {
        variable: name,
        arguments,
        block,
        return_type_ref,
        return_ty: ()
    }.into_statement(start, end))
}

fn match_function(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Fn);

    let mut method = match_method(parser)?;
    method.start = start;

    Ok(method)
}

fn match_if(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::If);
    let condition = match_expression(parser)?;
    let block = expect_block(parser)?;

    let mut alternate: Option<Box<ASTStatement>> = None;

    if let Ok(_) = match_token!(parser, TokenKind::Keyword(KeywordKind::Else)) {
        if matches!(parser.peek(), Some(Token { kind: TokenKind::Keyword(KeywordKind::If), .. })) {
            alternate = Some(Box::new(match_if(parser)?))
        } else {
            let block = expect_block(parser)?;
            alternate = Some(Box::new(ASTStatementKind::Block(block.clone()).into_statement(block.start, block.end)));
        }
    }

    Ok(ASTStatementKind::If {
        condition,
        block,
        alternate
    }.into_statement(start, parser.get_token_at(parser.cursor - 1).end))
}

fn match_let(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Let);

    let descriptor = VariableDescriptor::Identifier(expect_identifier!(parser));

    let mut type_ref = None;

    if matches!(parser.peek(), Some(Token { kind: TokenKind::Colon, .. })) {
        parser.advance();

        type_ref = Some(match_type_ref(parser)?);
    }

    expect_token!(parser, TokenKind::Assign);

    let expression = match_expression(parser)?;
    
    let end = expression.end;

    Ok(ASTStatementKind::VariableDefinition { descriptor, expression, type_ref, ty: () }.into_statement(start, end))

    // if let Expression { kind: ExpressionKind::Assignment { left, operator, right, .. }, end, .. } = match_expression(parser)? {
    //     Ok(StatementKind::Expression {
    //         expression: ExpressionKind::Assignment { left, operator, right, is_definition: true }.into_expression(start, end)
    //     }.into_statement(start, end))
    // } else {
    //     let Token { start, end, .. } = parser.get_token_at(parser.cursor - 1);
        
    //     Err(LanguageError::with_source(ParserError::UnexpectedToken("'identifier'".into()), start, end))
    // }
}

fn match_export(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Export);

    let Some(statement) = match_statement(parser)? else {
        return Err(LanguageError::from(ParserError::UnexpectedEndOfInput))
    };

    let end = statement.end;

    Ok(ASTStatementKind::Export {
        declaration: Box::new(statement)
    }.into_statement(start, end))
}

fn match_for(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::For);
    let loop_variable = expect_identifier!(parser);
    expect_keyword!(parser, KeywordKind::In);

    let left = match parser.peek() {
        Some(Token { kind: TokenKind::NumericLiteral { .. } | TokenKind::Identifier { .. } | TokenKind::OpeningParenthesis | TokenKind::OpeningSquareBracket, .. }) => Some(match_expression(parser)?),
        _ => None
    };
    
    let iterable = match parser.peek().cloned() {
        Some(Token { kind: TokenKind::Range, start: range_start, end: range_end }) => {
            parser.advance();

            let right = match parser.peek() {
                Some(Token { kind: TokenKind::NumericLiteral { .. } | TokenKind::Identifier { .. } | TokenKind::OpeningParenthesis, .. }) => Some(match_expression(parser)?),
                _ => None
            };

            match (&left, &right) {
                (None, None) => return Err(LanguageError::with_source(ParserError::InvalidUsage("Both sides of a range expression cannot be None".into()), range_start, range_end)),
                _ => ()
            }

            Iterable::Range(left, right)
        }
        _ => {
            if let Some(expression) = left {
                Iterable::Array(expression)
            } else {
                return parser.error()
            }
        }
    };

    let block = expect_block(parser)?;

    Ok(ASTStatementKind::For {
        variable: loop_variable,
        iterable,
        statements: block.statements
    }.into_statement(start, block.end))
}

fn match_while(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::While);
    let condition = match_expression(parser)?;
    let block = expect_block(parser)?;

    Ok(ASTStatementKind::While {
        condition,
        statements: block.statements
    }.into_statement(start, block.end))
}

fn match_class(parser: &mut Parser) -> LanguageResult<ASTStatement> {
    let Token { start, .. } = expect_keyword!(parser, KeywordKind::Class);
    let name = expect_identifier!(parser);
    
    let mut methods: Vec<_> = vec![];
    let mut fields: Vec<_> = vec![];

    let Token { end: body_start, .. } = expect_token!(parser, TokenKind::OpeningBracket);
    
    let body_end: usize;

    loop {
        if let Ok(Token { start, .. }) = match_token!(parser, TokenKind::ClosingBracket) {
            body_end = start;
            break;
        }
 
        if let Ok(_) = match_token!(parser, TokenKind::Keyword(KeywordKind::Fn)) {
            parser.cursor -= 1;
            methods.push(match_function(parser)?);
        } else if let Ok(_) = match_token!(parser, TokenKind::Identifier { .. }) {
            parser.cursor -= 1;

            let method = match_method(parser)?;

            let ASTStatement { kind: ASTStatementKind::Function { return_type_ref, .. }, .. } = &method else {
                unreachable!()
            };

            if let Some(return_type_ref) = return_type_ref {
                let span = return_type_ref.get_span();
                return Err(LanguageError::with_source(ParserError::InvalidUsage("Constructor cannot have a return type annotation".to_string()), span.start, span.end))
            }

            methods.push(method);
        } else {
            let (start, visibility) = match expect_token!(parser, TokenKind::Keyword(KeywordKind::Public | KeywordKind::Protected | KeywordKind::Private)) {
                Token { kind: TokenKind::Keyword(keyword), start, .. } => (start, Visibility::from(&TokenKind::Keyword(keyword))),
                _ => unreachable!()
            };

            let Identifier { value: field_name, .. } = expect_identifier!(parser);

            expect_token!(parser, TokenKind::Assign);
            let expression = parse_expression(parser, 0)?;
            let end = expression.end;

            fields.push(ASTStatementKind::Field {
                name: field_name,
                value: expression,
                visibility
            }.into_statement(start, end));
        }
    }

    Ok(ASTStatementKind::Class {
        variable: name,
        methods,
        fields,
        body_start,
        body_end
    }.into_statement(start, parser.get_token_at(parser.cursor - 1).end))
}

fn match_expression(parser: &mut Parser) -> LanguageResult<Expression> {
    parse_expression(parser, 0)
}

fn match_type_ref(parser: &mut Parser) -> LanguageResult<TypeRef> {
    let (mut type_ref, start) = match parser.peek().cloned() {
        Some(Token { kind: TokenKind::Identifier { .. }, start, .. }) => {
            (TypeRef::Named(expect_identifier!(parser)), start)
        }
        Some(Token { kind: TokenKind::OpeningBracket, start, .. }) => {
            let mut entries: IndexMap<String, TypeRef> = IndexMap::new();

            expect_comma_separated_group!(parser, TokenKind::OpeningBracket, TokenKind::ClosingBracket, {
                let identifier = expect_identifier!(parser);

                expect_token!(parser, TokenKind::Colon);

                let type_ref = match_type_ref(parser)?;

                entries.insert(identifier.value, type_ref);
            });

            let Token { end, .. } = parser.get_token_at(parser.cursor - 1);

            (TypeRef::Object { entries, span: Span { start, end } }, start)
        }
        _ => unreachable!()
    };

    while matches!(parser.peek(), Some(Token { kind: TokenKind::OpeningSquareBracket, .. })) {
        parser.advance();

        let Token { end, .. } = expect_token!(parser, TokenKind::ClosingSquareBracket);

        type_ref = TypeRef::Array { type_ref: Box::new(type_ref), span: Span { start, end } }
    }

    Ok(type_ref)
}