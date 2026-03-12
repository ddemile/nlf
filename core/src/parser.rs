use std::{
    cell::RefCell, collections::HashMap, fmt::{self, Debug}, rc::Rc, sync::Arc
};

use serde::Serialize;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};

use crate::{
    errors::{LanguageError, LanguageErrorTrait, LanguageResult},
    interpreter::{ClassDefinition, ModuleContext, Scope, prototypes::Method},
    lexer::{KeywordKind, Token, TokenKind},
};

macro_rules! expect_token {
    ($pat:pat, $expected:expr, $token:expr) => {{
        let Some(token) = $token else {
            panic!("Unexpected end of input while expecting '{}'", $expected);
        };

        if !matches!(token.kind, $pat) {
            return Err(LanguageError::with_source(
                ParserError::UnexpectedToken($expected.into()),
                token.start,
                token.end,
            ));
        }
    }};
}

#[derive(Debug)]
pub enum ParserError {
    UnexpectedToken(String),
    InvalidType(String),
}

impl LanguageErrorTrait for ParserError {}

#[derive(Serialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct ObjectRef {
    pub object_id: usize,
}

#[derive(Serialize, Debug, Clone)]
pub struct ArrayRef {
    pub array_id: usize,
}

#[derive(Serialize, Debug, Clone)]
pub struct RuntimeFunction {
    pub arguments: Vec<VariableRef>,
    pub statements: Rc<[Statement]>,
    #[serde(skip)]
    pub scope: Rc<RefCell<Scope>>,
    #[serde(skip)]
    pub context: *mut ModuleContext,  // ← add this back, only here
}

#[derive(Serialize, Clone)]
pub struct BuiltInFunction {
    #[serde(skip)]
    pub func: Method,
    #[serde(skip)]
    pub instance: Rc<ValueHolder>,
}

impl Debug for BuiltInFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function").finish()
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum FunctionKind {
    Runtime(RuntimeFunction),
    BuiltIn(BuiltInFunction),
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum ValueHolder {
    String(String),
    Number(DynamicNumber),
    Bool(bool),
    Fn(FunctionKind),
    Object(ObjectRef),
    Array(ArrayRef),
    #[serde(skip)]
    LazyRef {
        slot: usize,
        module: String,
    },
    ClassDefinition(ClassDefinition),
    Void,
}

impl PartialEq for ValueHolder {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueHolder::String(a), ValueHolder::String(b)) => a == b,
            (ValueHolder::Number(a), ValueHolder::Number(b)) => a == b,
            (ValueHolder::Bool(a), ValueHolder::Bool(b)) => a == b,
            (ValueHolder::Void, ValueHolder::Void) => true,
            _ => false, // different variants are never equal
        }
    }
}

impl PartialOrd for ValueHolder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (ValueHolder::Number(a), ValueHolder::Number(b)) => a.partial_cmp(b),
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
            ValueHolder::Number(a) => a > DynamicNumber::new(NumberHolder::Unsigned8(0)),
            ValueHolder::String(a) => a.len() > 0,
            ValueHolder::Bool(a) => a,
            _ => false, // different variants cannot be compared
        }
    }
}

impl Into<i32> for ValueHolder {
    fn into(self) -> i32 {
        match self {
            ValueHolder::Number(a) => a.into(),
            _ => panic!("Couldn't convert ValueHolder into i32"),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")] // "type" field will contain the variant name
pub struct Block {
    pub statements: Rc<[Statement]>,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportSpecifier {
    pub local: Expression,
}

#[derive(Serialize, Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub start: usize,
    pub end: usize,
}

impl StatementKind {
    fn into_statement(self, start: usize, end: usize) -> Statement {
        Statement { kind: self, start, end }
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum StatementKind {
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
        statements: Rc<[Statement]>,
    },
    ForIR {
        variable: VariableRef,
        left: Option<Expression>,
        right: Option<Expression>,
        statements: Rc<[Statement]>,
    },
    Function {
        name: String,
        arguments: Vec<Argument>,
        statements: Rc<[Statement]>,
    },
    FunctionIR {
        var_ref: VariableRef,
        arguments: Vec<VariableRef>,
        statements: Rc<[Statement]>,
    },
    While {
        condition: Expression,
        statements: Rc<[Statement]>,
    },
    Return {
        expression: Expression,
    },
    Break,
    Block(Block),
    Import {
        specifiers: Vec<ImportSpecifier>,
        source: String,
    },
    ImportIR {
        specifiers: Vec<VariableRef>,
        source: String,
    },
    Export {
        declaration: Box<Statement>,
    },
    Class {
        name: String,
        methods: Vec<Statement>,
        fields: Vec<Statement>,
    },
    ClassIR {
        var_ref: VariableRef,
        methods: Vec<Statement>,
        fields: Vec<Statement>,
    },
    Field {
        visibility: Visibility,
        name: String,
        value: Expression,
    },
    Method {
        name: String,
        arguments: Vec<VariableRef>,
        statements: Rc<[Statement]>,
    },
}

#[derive(Serialize, Debug, Clone)]
pub enum LiteralExpressionKind {
    Literal,
    Variable,
    Object(HashMap<String, Expression>),
    Array(Vec<Expression>),
    Function(Box<StatementKind>)
}

#[derive(Serialize, Debug, Clone)]
pub struct VariableRef {
    pub name: Option<String>,
    pub slot: usize,
    pub depth: usize,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "#type")] // "type" field will contain the variant name
pub enum ExpressionKind {
    Literal {
        r#type: LiteralExpressionKind,
        value: ValueHolder,
    },
    Unary {
        left: Box<Expression>,
        operator: TokenKind,
    },
    Variable(VariableRef),
    Member {
        object: Box<Expression>,
        property: Box<Expression>,
    },
    Binary {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    Relational {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    Logical {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    Equality {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Assignment {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
        is_definition: bool,
    },
}

impl ExpressionKind {
    pub fn into_expression(self, start: usize, end: usize) -> Expression {
        Expression { kind: self, start, end }
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum Visibility {
    Public,
    Protected,
    Private,
}

impl Visibility {
    pub fn from(kind: &TokenKind) -> Self {
        match kind {
            TokenKind::Keyword(KeywordKind::Public) => Visibility::Public,
            TokenKind::Keyword(KeywordKind::Protected) => Visibility::Protected,
            TokenKind::Keyword(KeywordKind::Private) => Visibility::Private,
            _ => panic!("Expected a valid visibility keyword"),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")] // "type" field will contain the variant name
pub struct Program {
    pub body: Rc<[Statement]>,
}

fn parse_internal(mut tokens: Vec<Token>) -> LanguageResult<(Rc<[Statement]>, usize)> {
    let mut cursor = 0;
    let mut statements: Vec<Statement> = vec![];

    while cursor < tokens.len() {
        let tokens_copy = tokens.clone();
        let token = tokens_copy.get(cursor).unwrap();

        match match_token(&mut cursor, &mut tokens, token.clone())? {
            Some(statement) => statements.push(statement),
            None => break,
        }
    }

    Ok((statements.into(), cursor))
}

pub fn parse(tokens: Vec<Token>) -> LanguageResult<Program> {
    Ok(Program {
        body: parse_internal(tokens)?.0,
    })
}

fn match_token(
    cursor: &mut usize,
    tokens: &mut Vec<Token>,
    token: Token,
) -> LanguageResult<Option<Statement>> {
    let start = tokens.get(*cursor).unwrap().start;
    Ok(match token.kind {
        TokenKind::Keyword(KeywordKind::If) => {
            *cursor += 1;

            Some(parse_if(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::For) => {
            *cursor += 1;

            expect_token!(
                TokenKind::Identifier { .. },
                "variable literal",
                tokens.get(*cursor)
            );

            let loop_variable = literal_expression(cursor, tokens)?;

            expect_token!(
                TokenKind::Keyword(KeywordKind::In),
                "'in' keyword",
                tokens.get(*cursor)
            );

            *cursor += 1;

            let left = match tokens.get(*cursor).map(|t| &t.kind) {
                Some(TokenKind::NumericLiteral { .. })
                | Some(TokenKind::Identifier { .. })
                | Some(TokenKind::OpeningParenthesis) => Some(unary_expression(cursor, tokens)?),
                _ => None,
            };

            expect_token!(TokenKind::Range, "range (..) operator", tokens.get(*cursor));

            *cursor += 1;

            let right = match tokens.get(*cursor).map(|t| &t.kind) {
                Some(TokenKind::NumericLiteral { .. })
                | Some(TokenKind::Identifier { .. })
                | Some(TokenKind::OpeningParenthesis) => Some(unary_expression(cursor, tokens)?),
                _ => None,
            };

            let block = block(cursor, tokens)?;

            match (&left, &right) {
                (None, None) => panic!("Both sides of a range expression cannot be None"),
                _ => {}
            }

            Some(StatementKind::For {
                variable: loop_variable,
                left,
                right,
                statements: block.statements,
            }.into_statement(start, block.end))
        }
        TokenKind::Keyword(KeywordKind::While) => {
            *cursor += 1;

            let condition = logical_expression(cursor, tokens)?;

            let block = block(cursor, tokens)?;

            Some(StatementKind::While {
                condition,
                statements: block.statements,
            }.into_statement(start, block.end))
        }
        TokenKind::Keyword(KeywordKind::Fn) => {
            *cursor += 1;

            Some(parse_function(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::Return) => {
            *cursor += 1;
            Some(StatementKind::Return {
                expression: logical_expression(cursor, tokens)?,
            }.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
        }
        TokenKind::Keyword(KeywordKind::Break) => {
            *cursor += 1;
            Some(StatementKind::Break.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
        }
        TokenKind::Keyword(KeywordKind::Class) => {
            *cursor += 1;

            Some(parse_class(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::Import) => {
            *cursor += 1;
            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::OpeningBracket)
            ) {
                let token = tokens.get(*cursor - 1).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'{{'".into()),
                    token.end,
                    token.end,
                ));
            }
            *cursor += 1;

            let mut specifiers: Vec<ImportSpecifier> = vec![];

            while let Some(token) = tokens.get(*cursor) {
                match token.kind {
                    TokenKind::ClosingBracket => {
                        // End of argument list
                        break;
                    }
                    _ => {
                        // Parse the argument
                        let expr = logical_expression(cursor, tokens)?;
                        specifiers.push(ImportSpecifier { local: expr });

                        // After parsing argument, check if next token is a comma
                        match tokens.get(*cursor).map(|token| token.kind.clone()) {
                            Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                            Some(TokenKind::ClosingBracket) => break, // done
                            _ => panic!("Expected ',' or '}}' after argument"),
                        }
                    }
                }
            }

            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingBracket)
            ) {
                let token = tokens.get(*cursor - 1).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'{{'".into()),
                    token.end,
                    token.end,
                ));
            }

            *cursor += 1;

            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::Keyword(KeywordKind::From))
            ) {
                let token = tokens.get(*cursor - 1).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'from' keyword".into()),
                    token.end,
                    token.end,
                ));
            }

            *cursor += 1;

            let source = literal_expression(cursor, tokens)?;

            let ExpressionKind::Literal {
                value: ValueHolder::String(source),
                ..
            } = source.kind
            else {
                let token = tokens.get(*cursor - 1).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::InvalidType("string literal".into()),
                    token.start,
                    token.end,
                ));
            };

            Some(StatementKind::Import { specifiers, source }.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
        }
        TokenKind::Keyword(KeywordKind::Export) => {
            *cursor += 1;

            let statement = match_token(cursor, tokens, tokens.get(*cursor).unwrap().clone())?;

            let Some(statement) = statement else {
                return Ok(None);
            };

            Some(StatementKind::Export {
                declaration: Box::new(statement),
            }.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
        }
        TokenKind::ClosingBracket => None,
        _ => Some(StatementKind::Expression {
            expression: left(cursor, tokens)?,
        }.into_statement(start, tokens.get(*cursor - 1).unwrap().end)),
    })
}

fn parse_class(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let start = tokens.get(*cursor).unwrap().start;

    let Some(Token {
        kind: TokenKind::Identifier { value },
        ..
    }) = tokens.get(*cursor)
    else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(
            ParserError::InvalidType("identifier".into()),
            token.start,
            token.end,
        ));
    };

    let name = value.to_string();

    *cursor += 1;

    let Some(Token {
        kind: TokenKind::OpeningBracket,
        ..
    }) = tokens.get(*cursor)
    else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(
            ParserError::UnexpectedToken("'{{'".into()),
            token.start,
            token.end,
        ));
    };

    *cursor += 1;

    let mut methods: Vec<Statement> = vec![];
    let mut fields: Vec<Statement> = vec![];

    while !matches!(
        tokens.get(*cursor),
        Some(Token {
            kind: TokenKind::ClosingBracket,
            ..
        })
    ) {
        let token = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        if let TokenKind::Identifier { value } = &token.kind {
            if *value == name {
                *cursor -= 1;
                methods.push(parse_function(cursor, tokens)?);
                continue;
            }
        }

        if token.kind == TokenKind::Keyword(KeywordKind::Fn) {
            let Some(Token {
                kind: TokenKind::Identifier { value },
                ..
            }) = tokens.get(*cursor)
            else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::InvalidType("identifier".into()),
                    token.start,
                    token.end,
                ));
            };

            if *value == name {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("a valid function name".into()),
                    token.start,
                    token.end,
                ));
            }

            methods.push(parse_function(cursor, tokens)?);
        } else if let TokenKind::Keyword(
            KeywordKind::Public | KeywordKind::Protected | KeywordKind::Private,
        ) = &token.kind
        {
            let visibility = Visibility::from(&token.kind);

            let Some(Token {
                kind: TokenKind::Identifier { value: field_name },
                ..
            }) = tokens.get(*cursor).cloned()
            else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'a valid field name'".into()),
                    token.start,
                    token.end,
                ));
            };

            *cursor += 1;

            let Some(Token {
                kind: TokenKind::Assign,
                ..
            }) = tokens.get(*cursor)
            else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'='".into()),
                    token.start,
                    token.end,
                ));
            };

            *cursor += 1;

            let value = logical_expression(cursor, tokens)?;

            fields.push(StatementKind::Field {
                visibility,
                name: field_name,
                value,
            }.into_statement(token.start, tokens.get(*cursor - 1).unwrap().end));
        }
    }

    *cursor += 1;

    Ok(StatementKind::Class {
        name,
        methods,
        fields,
    }.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
}

fn parse_function(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let start = tokens.get(*cursor).unwrap().start;

    let Some(Token {
        kind: TokenKind::Identifier { value },
        ..
    }) = tokens.get(*cursor)
    else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(
            ParserError::InvalidType("identifier".into()),
            token.start,
            token.end,
        ));
    };

    let value = value.clone();

    if matches!(
        tokens.get(*cursor + 1).map(|token| token.kind.clone()),
        Some(TokenKind::OpeningParenthesis)
    ) {
        *cursor += 2; // move to the first character after (

        let mut args: Vec<Argument> = vec![];

        while let Some(token) = tokens.get(*cursor) {
            match token.kind {
                TokenKind::ClosingParenthesis => {
                    // End of argument list
                    break;
                }
                _ => {
                    // Parse the argument
                    let expr = literal_expression(cursor, tokens)?;

                    let ExpressionKind::Literal {
                        r#type: LiteralExpressionKind::Variable,
                        value: ValueHolder::String(name),
                    } = expr.kind
                    else {
                        panic!();
                    };

                    args.push(Argument { name });

                    // After parsing argument, check if next token is a comma
                    match tokens.get(*cursor).map(|token| token.kind.clone()) {
                        Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                        Some(TokenKind::ClosingParenthesis) => break, // done
                        _ => {
                            let token = tokens.get(*cursor - 1).unwrap();
                            return Err(LanguageError::with_source(
                                ParserError::UnexpectedToken("',' or ')' after argument".into()),
                                token.end,
                                token.end,
                            ));
                        },
                    }
                }
            }
        }

        if !matches!(
            tokens.get(*cursor).map(|token| token.kind.clone()),
            Some(TokenKind::ClosingParenthesis)
        ) {
            let token = tokens.get(*cursor - 1).unwrap();
            return Err(LanguageError::with_source(
                ParserError::UnexpectedToken("')'".into()),
                token.end,
                token.end,
            ));
        }

        *cursor += 1; // move past ')'

        let block = block(cursor, tokens)?;

        return Ok(StatementKind::Function {
            name: value.to_string(),
            arguments: args,
            statements: block.statements,
        }.into_statement(start, tokens.get(*cursor - 1).unwrap().end));
    }

    let token = tokens.get(*cursor).unwrap();
    Err(LanguageError::with_source(
        ParserError::UnexpectedToken("'('".into()),
        token.end,
        token.end,
    ))
}

fn parse_if(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let start = tokens.get(*cursor).unwrap().start;

    let condition = logical_expression(cursor, tokens)?;

    let inner_block = block(cursor, tokens)?;

    let mut alternate: Option<Box<Statement>> = None;

    if let Some(Token {
        kind: TokenKind::Keyword(KeywordKind::Else),
        ..
    }) = tokens.get(*cursor)
    {
        *cursor += 1;
        if let Some(Token {
            kind: TokenKind::Keyword(KeywordKind::If),
            ..
        }) = tokens.get(*cursor)
        {
            *cursor += 1;
            alternate = Some(Box::new(parse_if(cursor, tokens)?));
        } else {
            let block = block(cursor, tokens)?;
            alternate = Some(Box::new(StatementKind::Block(block.clone()).into_statement(block.start, block.end)));
        }
    }

    Ok(StatementKind::If {
        condition,
        block: inner_block,
        alternate,
    }.into_statement(start, tokens.get(*cursor - 1).unwrap().end))
}

fn block(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Block> {
    if !matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::OpeningBracket)
    ) {
        let token = tokens.get(*cursor - 1).unwrap();
        return Err(LanguageError::with_source(
            ParserError::UnexpectedToken("'{{'".into()),
            token.end,
            token.end,
        ));
    }

    let start = tokens.get(*cursor).unwrap().start;

    *cursor += 1;
    let (inner_statements, inner_cursor) = parse_internal(tokens[*cursor..].to_vec())?;
    *cursor += inner_cursor;

    if !matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::ClosingBracket)
    ) {
        let token = tokens.get(*cursor - 1).unwrap();
        return Err(LanguageError::with_source(
            ParserError::UnexpectedToken("'}}'".into()),
            token.end,
            token.end,
        ));
    }

    *cursor += 1;

    Ok(Block { statements: inner_statements, start, end: tokens.get(*cursor - 1).unwrap().end })
}

fn left(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    return assignment_expression(cursor, tokens);
}

fn assignment_expression(
    cursor: &mut usize,
    tokens: &mut Vec<Token>,
) -> LanguageResult<Expression> {
    let start = tokens.get(*cursor).unwrap().start;

    let is_definition = if let Some(Token {
        kind: TokenKind::Keyword(KeywordKind::Let),
        ..
    }) = tokens.get(*cursor)
    {
        *cursor += 1;
        true
    } else {
        false
    };

    let left = logical_expression(cursor, tokens)?;

    if matches!(left.kind, ExpressionKind::Literal { .. })
        || (!is_definition && matches!(left.kind, ExpressionKind::Member { .. }))
    {
        if let Some(Token {
            kind:
                TokenKind::Assign
                | TokenKind::PlusEqual
                | TokenKind::MinusEqual
                | TokenKind::AsteriskEqual
                | TokenKind::SlashEqual
                | TokenKind::PercentEqual,
            ..
        }) = tokens.get(*cursor)
        {
            let operator = tokens.get(*cursor).unwrap().kind.clone();
            *cursor += 1;
            let right = Box::new(logical_expression(cursor, tokens)?);
            return Ok(ExpressionKind::Assignment {
                left: Box::new(left),
                operator,
                right: right.clone(),
                is_definition,
            }.into_expression(start, right.end));
        }
    }

    Ok(left)
}

fn logical_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = equality_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::And | TokenKind::Or)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        let right = Box::new(equality_expression(cursor, tokens)?);
        
        let start = left.start;
        let end = right.end;

        left = ExpressionKind::Logical {
            left: Box::new(left),
            operator,
            right: right.clone(),
        }.into_expression(start, end);
    }

    Ok(left)
}

fn equality_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = relational_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::EQ | TokenKind::NE)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        let right = relational_expression(cursor, tokens)?;
        
        let start = left.start;
        let end = right.end;

        left = ExpressionKind::Equality {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }.into_expression(start, end);
    }

    Ok(left)
}

fn relational_expression(
    cursor: &mut usize,
    tokens: &mut Vec<Token>,
) -> LanguageResult<Expression> {
    let mut left = term_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::GT | TokenKind::GTE | TokenKind::LT | TokenKind::LTE)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        let right = term_expression(cursor, tokens)?;

        let start = left.start;
        let end = right.end;

        left = ExpressionKind::Relational {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }.into_expression(start, end);
    }

    Ok(left)
}

fn term_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = factor_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::Plus | TokenKind::Minus)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        let right = factor_expression(cursor, tokens)?;

        let start = left.start;
        let end = right.end;

        left = ExpressionKind::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }.into_expression(start, end);
    }

    Ok(left)
}

fn factor_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = call_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::Asterisk | TokenKind::Slash | TokenKind::Percent)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        let right = call_expression(cursor, tokens)?;

        let start = left.start;
        let end = right.end;

        left = ExpressionKind::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }.into_expression(start, end);
    }

    Ok(left)
}

fn call_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    if let Some(Token {
        kind: TokenKind::Identifier { value },
        start,
        end
    }) = tokens.get(*cursor)
    {
        let start = start.clone();
        let end = end.clone();

        if matches!(
            tokens.get(*cursor + 1).map(|token| token.kind.clone()),
            Some(TokenKind::OpeningParenthesis)
        ) {
            let name = value.clone();
            *cursor += 1; // move to the '('

            *cursor += 1; // move to the first token inside parentheses
            let mut args: Vec<Expression> = vec![];

            while let Some(token) = tokens.get(*cursor) {
                match token.kind {
                    TokenKind::ClosingParenthesis => {
                        // End of argument list
                        break;
                    }
                    _ => {
                        // Parse the argument
                        let expr = logical_expression(cursor, tokens)?;
                        args.push(expr);

                        // After parsing argument, check if next token is a comma
                        match tokens.get(*cursor).map(|token| token.kind.clone()) {
                            Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                            Some(TokenKind::ClosingParenthesis) => break, // done
                            _ => panic!("Expected ',' or ')' after argument"),
                        }
                    }
                }
            }

            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingParenthesis)
            ) {
                panic!("Expected ')'");
            }

            *cursor += 1; // move past ')'

            return Ok(ExpressionKind::Call {
                callee: Box::new(ExpressionKind::Literal {
                    r#type: LiteralExpressionKind::Variable,
                    value: ValueHolder::String(name),
                }.into_expression(start, end)),
                arguments: args,
            }.into_expression(start, tokens.get(*cursor - 1).unwrap().end));
        }
    }

    unary_expression(cursor, tokens)
}

fn unary_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    if matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::Plus) | Some(TokenKind::Minus)
    ) {
        let token = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        return Ok(ExpressionKind::Unary {
            left: Box::new(literal_expression(cursor, tokens)?),
            operator: token.kind,
        }.into_expression(token.start, token.end));
    }

    literal_expression(cursor, tokens)
}

fn literal_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let token = tokens.get(*cursor).unwrap().clone();

    *cursor += 1;

    let expr = match &token.kind {
        TokenKind::NumericLiteral { value } => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Number(DynamicNumber::from_str(value)),
        },
        TokenKind::BooleanLiteral { value } => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Bool(*value),
        },
        TokenKind::StringLiteral { value } => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::String(value.clone()),
        },
        TokenKind::Identifier { value } => ExpressionKind::Literal {
            r#type: LiteralExpressionKind::Variable,
            value: ValueHolder::String(value.clone()),
        },
        TokenKind::OpeningParenthesis => {
            let start_cursor = cursor.clone();

            if let Ok(args) = {
                match match_arguments(cursor, tokens) {
                    Ok(args) => {
                        *cursor += 1;

                        if matches!(
                            tokens.get(*cursor).map(|token| token.kind.clone()),
                            Some(TokenKind::Arrow)
                        ) {
                            Ok(args)
                        } else {
                            Err(LanguageError::from(ParserError::UnexpectedToken("'=>'".into())))
                        }
                    }
                    Err(err) => Err(err)
                }
            } {
                *cursor += 1;

                let block = block(cursor, tokens)?;

                return Ok(ExpressionKind::Literal { r#type: LiteralExpressionKind::Function(Box::new(StatementKind::Function {
                    name: "<lambda>".to_string(),
                    arguments: args,
                    statements: block.statements
                })), value: ValueHolder::Void }.into_expression(tokens.get(start_cursor).unwrap().start, tokens.get(*cursor - 1).unwrap().end));
            }

            *cursor = start_cursor;

            let expr = left(cursor, tokens)?;
            
            let Some(Token { kind: TokenKind::ClosingParenthesis, .. }) = tokens.get(*cursor) else {
                let token = tokens.get(*cursor - 1).unwrap();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("'}}'".into()),
                    token.end,
                    token.end,
                ));
            };

            *cursor += 1;

            expr.kind
        }
        TokenKind::OpeningBracket => {
            let mut entries: HashMap<String, Expression> = HashMap::new();
            while !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingBracket)
            ) {
                let key = literal_expression(cursor, tokens)?;

                let ExpressionKind::Literal {
                    value: ValueHolder::String(key),
                    ..
                } = key.kind
                else {
                    todo!();
                };

                let Some(Token {
                    kind: TokenKind::Colon,
                    ..
                }) = tokens.get(*cursor)
                else {
                    todo!();
                };

                *cursor += 1;

                let value = literal_expression(cursor, tokens)?;

                entries.insert(key, value);

                match tokens.get(*cursor).map(|token| token.kind.clone()) {
                    Some(TokenKind::Comma) => {
                        *cursor += 1;
                        continue;
                    }
                    Some(TokenKind::ClosingBracket) => {
                        break;
                    }
                    _ => todo!(),
                }
            }

            *cursor += 1;

            ExpressionKind::Literal {
                r#type: LiteralExpressionKind::Object(entries),
                value: ValueHolder::Void,
            }
        }
        TokenKind::OpeningSquareBracket => {
            let mut items: Vec<Expression> = vec![];

            while !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingSquareBracket)
            ) {
                let item = logical_expression(cursor, tokens)?;

                items.push(item);

                match tokens.get(*cursor).map(|token| token.kind.clone()) {
                    Some(TokenKind::Comma) => {
                        *cursor += 1;
                        continue;
                    }
                    Some(TokenKind::ClosingSquareBracket) => {
                        break;
                    }
                    _ => todo!(),
                }
            }

            *cursor += 1;

            ExpressionKind::Literal {
                r#type: LiteralExpressionKind::Array(items),
                value: ValueHolder::Void,
            }
        }
        _ => {
            return Err(LanguageError::with_source(
                ParserError::UnexpectedToken("function argument".into()),
                token.start,
                token.end,
            ));
        }
    };

    member(cursor, tokens, expr.into_expression(token.start, tokens.get(*cursor - 1).unwrap().end), true)
}

fn member(
    cursor: &mut usize,
    tokens: &mut Vec<Token>,
    mut expr: Expression,
    match_call: bool,
) -> LanguageResult<Expression> {
    loop {
        if matches!(
            tokens.get(*cursor).map(|token| token.kind.clone()),
            Some(TokenKind::Period)
        ) {
            *cursor += 1;
            let Some(Token {
                kind: TokenKind::Identifier { value },
                start: identifier_start,
                end: identifier_end
            }) = tokens.get(*cursor).cloned()
            else {
                panic!()
            };

            *cursor += 1;

            let start = expr.start;

            expr = ExpressionKind::Member {
                object: Box::new(expr),
                property: Box::new(ExpressionKind::Literal {
                    r#type: LiteralExpressionKind::Literal,
                    value: ValueHolder::String(value.to_string()),
                }.into_expression(identifier_start, identifier_end)),
            }.into_expression(start, identifier_end);
        } else if matches!(
            tokens.get(*cursor).map(|token| token.kind.clone()),
            Some(TokenKind::OpeningSquareBracket)
        ) {
            *cursor += 1;
            
            let property = logical_expression(cursor, tokens)?;
            let start = expr.start;
            let end = property.end;

            expr = ExpressionKind::Member {
                object: Box::new(expr),
                property: Box::new(property),
            }.into_expression(start, end);

            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingSquareBracket)
            ) {
                panic!()
            }

            *cursor += 1;
        } else {
            break;
        }

        // Check if call
        if match_call
            && matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::OpeningParenthesis)
            )
        {
            *cursor += 1; // move to the first token inside parentheses
            let mut args: Vec<Expression> = vec![];

            while let Some(token) = tokens.get(*cursor).cloned() {
                match token.kind {
                    TokenKind::ClosingParenthesis => {
                        // End of argument list
                        break;
                    }
                    _ => {
                        // Parse the argument
                        let expr = logical_expression(cursor, tokens)?;
                        args.push(expr);

                        // After parsing argument, check if next token is a comma
                        match tokens.get(*cursor).map(|token| token.kind.clone()) {
                            Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                            Some(TokenKind::ClosingParenthesis) => break, // done
                            _ => {
                                return Err(LanguageError::with_source(
                                    ParserError::UnexpectedToken("',' or ')'".into()),
                                    token.start,
                                    token.end,
                                ));
                            }
                        }
                    }
                }
            }

            if !matches!(
                tokens.get(*cursor).map(|token| token.kind.clone()),
                Some(TokenKind::ClosingParenthesis)
            ) {
                let Token { start, end, .. } = tokens.get(*cursor).unwrap().clone();
                return Err(LanguageError::with_source(
                    ParserError::UnexpectedToken("',' or ')'".into()),
                    start,
                    end,
                ));
            }

            *cursor += 1; // move past ')'

            expr = ExpressionKind::Call {
                callee: Box::new(expr),
                arguments: args,
            }.into_expression(0, 0);
        }
    }

    Ok(expr)
}

fn match_arguments(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Vec<Argument>> {
    let mut args: Vec<Argument> = vec![];

    while let Some(token) = tokens.get(*cursor) {
        match token.kind {
            TokenKind::ClosingParenthesis => {
                // End of argument list
                break;
            }
            _ => {
                // Parse the argument
                let expr = literal_expression(cursor, tokens)?;

                let ExpressionKind::Literal {
                    r#type: LiteralExpressionKind::Variable,
                    value: ValueHolder::String(name),
                } = expr.kind
                else {
                    return Err(LanguageError::from(ParserError::InvalidType("Literal".into())))
                };

                args.push(Argument { name });

                // After parsing argument, check if next token is a comma
                match tokens.get(*cursor).map(|token| token.kind.clone()) {
                    Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                    Some(TokenKind::ClosingParenthesis) => break, // done
                    _ => return Err(LanguageError::from(ParserError::UnexpectedToken("',' or ')'".into()))),
                }
            }
        }
    }

    Ok(args)
}