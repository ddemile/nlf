use std::{
    cell::RefCell, collections::HashMap, fmt::{self, Debug}, rc::Rc, sync::{Arc}
};

use serde::Serialize;
use shared::numbers::{DynamicNumber, NumberHolder};

use crate::{
    errors::{LanguageError, LanguageErrorTrait, LanguageResult}, interpreter::{ClassDefinition, Scope, prototypes::Method}, lexer::{KeywordKind, Token, TokenKind}
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
    InvalidType(String)
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
    pub statements: Vec<Statement>,
    #[serde(skip)]
    pub scope: Rc<RefCell<Scope>>,
}

#[derive(Serialize, Clone)]
pub struct BuiltInFunction {
    #[serde(skip)]
    pub func: Method,
    #[serde(skip)]
    pub instance: Arc<ValueHolder>,
}

impl Debug for BuiltInFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function").finish()
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum FunctionKind {
    Runtime(RuntimeFunction),
    BuiltIn(BuiltInFunction)
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
    LazyRef { slot: usize, module: String },
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
    pub statements: Vec<Statement>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportSpecifier {
    pub local: Expression,
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
    While {
        condition: Expression,
        statements: Vec<Statement>,
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
        fields: Vec<Statement>
    },
    ClassIR {
        var_ref: VariableRef,
        methods: Vec<Statement>,
        fields: Vec<Statement>
    },
    Field {
        visibility: Visibility,
        name: String,
        value: Expression
    },
    Method {
        name: String,
        arguments: Vec<VariableRef>,
        statements: Vec<Statement>,
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum LiteralExpressionKind {
    Literal,
    Variable,
    Object(HashMap<String, Expression>),
    Array(Vec<Expression>),
}

#[derive(Serialize, Debug, Clone)]
pub struct VariableRef {
    pub name: Option<String>,
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
    Unary {
        left: Box<Expression>,
        operator: TokenKind
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

#[derive(Serialize, Debug, Clone)]
pub enum Visibility {
    Public,
    Protected,
    Private
}

impl Visibility {
    pub fn from(kind: &TokenKind) -> Self {
        match kind {
            TokenKind::Keyword(KeywordKind::Public) => Visibility::Public,
            TokenKind::Keyword(KeywordKind::Protected) => Visibility::Protected,
            TokenKind::Keyword(KeywordKind::Private) => Visibility::Private,
            _ => panic!("Expected a valid visibility keyword")
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")] // "type" field will contain the variant name
pub struct Program {
    pub body: Vec<Statement>,
}

fn parse_internal(mut tokens: Vec<Token>) -> LanguageResult<(Vec<Statement>, usize)> {
    let mut cursor = 0;
    let mut statements: Vec<Statement> = vec![];

    while cursor < tokens.len() {
        let tokens_copy = tokens.clone();
        let token = tokens_copy.get(cursor).unwrap();

        match match_token(&mut cursor, &mut tokens, token.clone())? {
            Some(statement) => statements.push(statement),
            None => break
        }
    }

    Ok((statements, cursor))
}

pub fn parse(tokens: Vec<Token>) -> LanguageResult<Program> {
    Ok(Program {
        body: parse_internal(tokens)?.0,
    })
}

fn match_token(cursor: &mut usize, tokens: &mut Vec<Token>, token: Token) -> LanguageResult<Option<Statement>> {
    Ok(match token.kind {
        TokenKind::Keyword(KeywordKind::If) => {
            *cursor += 1;

            Some(parse_if(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::For) => {
            *cursor += 1;

            expect_token!(TokenKind::Identifier { .. }, "variable literal", tokens.get(*cursor));

            let loop_variable = literal_expression(cursor, tokens)?;

            expect_token!(TokenKind::Keyword(KeywordKind::In), "'in' keyword", tokens.get(*cursor));

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

            let inner_statements = block(cursor, tokens)?;

            match (&left, &right) {
                (None, None) => panic!("Both sides of a range expression cannot be None"),
                _ => {}
            }

            Some(Statement::For {
                variable: loop_variable,
                left,
                right,
                statements: inner_statements,
            })
        }
        TokenKind::Keyword(KeywordKind::While) => {
            *cursor += 1;

            let condition = logical_expression(cursor, tokens)?;

            let inner_statements = block(cursor, tokens)?;

            Some(Statement::While {
                condition,
                statements: inner_statements,
            })
        }
        TokenKind::Keyword(KeywordKind::Fn) => {
            *cursor += 1;

            Some(parse_function(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::Return) => {
            *cursor += 1;
            Some(Statement::Return {
                expression: logical_expression(cursor, tokens)?,
            })
        }
        TokenKind::Keyword(KeywordKind::Break) => {
            *cursor += 1;
            Some(Statement::Break)
        }
        TokenKind::Keyword(KeywordKind::Class) => {
            *cursor += 1;
            
            Some(parse_class(cursor, tokens)?)
        }
        TokenKind::Keyword(KeywordKind::Import) => {
            *cursor += 1;
            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::OpeningBracket)) {
                panic!("Expected '{{'");
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
                            Some(TokenKind::Comma) => *cursor += 1,    // skip comma, continue loop
                            Some(TokenKind::ClosingBracket) => break, // done
                            _ => panic!("Expected ',' or '}}' after argument"),
                        }
                    }
                }
            }

            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingBracket)) {
                panic!("Expected }}")
            }

            *cursor += 1;

            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::Keyword(KeywordKind::From))) {
                panic!("Expected 'from' keyword")
            }

            *cursor += 1;

            let source = literal_expression(cursor, tokens)?;

            let Expression::Literal {
                value: ValueHolder::String(source),
                ..
            } = source
            else {
                panic!("Expected string as source")
            };

            Some(Statement::Import { specifiers, source })
        }
        TokenKind::Keyword(KeywordKind::Export) => {
            *cursor += 1;

            let statement = match_token(cursor, tokens, tokens.get(*cursor).unwrap().clone())?;

            let Some(statement) = statement else {
                return Ok(None)
            };

            Some(Statement::Export {
                declaration: Box::new(statement),
            })
        }
        TokenKind::ClosingBracket => None,
        _ => {
            Some(Statement::Expression {
                expression: left(cursor, tokens)?,
            })
        }
    })
}

fn parse_class(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let Some(Token { kind: TokenKind::Identifier { value }, .. }) = tokens.get(*cursor) else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(ParserError::InvalidType("identifier".into()), token.start, token.end))
    };

    let name = value.to_string();

    *cursor += 1;
    
    let Some(Token { kind: TokenKind::OpeningBracket, .. }) = tokens.get(*cursor) else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(ParserError::UnexpectedToken("'{{'".into()), token.start, token.end))
    };

    *cursor += 1;

    let mut methods: Vec<Statement> = vec![];
    let mut fields: Vec<Statement> = vec![];
    
    while !matches!(tokens.get(*cursor), Some(Token { kind: TokenKind::ClosingBracket, .. })) {
        let token = tokens.get(*cursor).unwrap();
        *cursor += 1;

        if let TokenKind::Identifier { value } = &token.kind {
            if *value == name {
                *cursor -= 1;
                methods.push(parse_function(cursor, tokens)?);
                continue;
            }
        }

        if token.kind == TokenKind::Keyword(KeywordKind::Fn) {
            let Some(Token { kind: TokenKind::Identifier { value }, .. }) = tokens.get(*cursor) else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(ParserError::InvalidType("identifier".into()), token.start, token.end))
            };

            if *value == name {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(ParserError::UnexpectedToken("a valid function name".into()), token.start, token.end))
            }

            methods.push(parse_function(cursor, tokens)?);
        } else if let TokenKind::Keyword(KeywordKind::Public | KeywordKind::Protected | KeywordKind::Private) = &token.kind {
            let visibility = Visibility::from(&token.kind);

            let Some(Token { kind: TokenKind::Identifier { value: field_name }, .. }) = tokens.get(*cursor).cloned() else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(ParserError::UnexpectedToken("'{{'".into()), token.start, token.end))
            };

            *cursor += 1;

            let Some(Token { kind: TokenKind::Assign, .. }) = tokens.get(*cursor) else {
                let token = tokens.get(*cursor).unwrap();
                return Err(LanguageError::with_source(ParserError::UnexpectedToken("'{{'".into()), token.start, token.end))
            };

            *cursor += 1;

            let value = logical_expression(cursor, tokens)?;

            fields.push(Statement::Field {
                visibility,
                name: field_name,
                value
            });
        }
    }

    *cursor += 1;

    Ok(Statement::Class {
        name,
        methods,
        fields
    })
}

fn parse_function(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let Some(Token { kind: TokenKind::Identifier { value }, .. }) = tokens.get(*cursor) else {
        let token = tokens.get(*cursor).unwrap();
        return Err(LanguageError::with_source(ParserError::InvalidType("identifier".into()), token.start, token.end))
    };

    let value = value.clone();

    if matches!(tokens.get(*cursor + 1).map(|token| token.kind.clone()), Some(TokenKind::OpeningParenthesis)) {
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

                    let Expression::Literal {
                        r#type: LiteralExpressionKind::Variable,
                        value: ValueHolder::String(name),
                    } = expr
                    else {
                        panic!();
                    };

                    args.push(Argument { name });

                    // After parsing argument, check if next token is a comma
                    match tokens.get(*cursor).map(|token| token.kind.clone()) {
                        Some(TokenKind::Comma) => *cursor += 1, // skip comma, continue loop
                        Some(TokenKind::ClosingParenthesis) => break, // done
                        _ => panic!("Expected ',' or ')' after argument"),
                    }
                }
            }
        }

        if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingParenthesis)) {
            panic!("Expected ')'");
        }

        *cursor += 1; // move past ')'

        let inner_statements = block(cursor, tokens)?;

        return Ok(Statement::Function {
            name: value.to_string(),
            arguments: args,
            statements: inner_statements,
        })
    }

    panic!("Expected '('");
}

fn parse_if(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Statement> {
    let condition = logical_expression(cursor, tokens)?;

    let inner_statements = block(cursor, tokens)?;

    let mut alternate: Option<Box<Statement>> = None;

    if let Some(Token { kind: TokenKind::Keyword(KeywordKind::Else), .. }) = tokens.get(*cursor) {
        *cursor += 1;
        if let Some(Token { kind: TokenKind::Keyword(KeywordKind::If), .. }) = tokens.get(*cursor) {
            *cursor += 1;
            alternate = Some(Box::new(parse_if(cursor, tokens)?));
        } else {
            alternate = Some(Box::new(Statement::Block(Block {
                statements: block(cursor, tokens)?,
            })));
        }
    }

    Ok(Statement::If {
        condition,
        block: Block {
            statements: inner_statements,
        },
        alternate,
    })
}

fn block(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Vec<Statement>> {
    if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::OpeningBracket)) {
        panic!("Expected {{")
    }
    *cursor += 1;
    let (inner_statements, inner_cursor) = parse_internal(tokens[*cursor..].to_vec())?;
    *cursor += inner_cursor;

    if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingBracket)) {
        panic!("Expected }}")
    }

    *cursor += 1;

    Ok(inner_statements)
}

fn left(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    return assignment_expression(cursor, tokens);
}

fn assignment_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let is_definition = if let Some(Token { kind: TokenKind::Keyword(KeywordKind::Let), .. }) = tokens.get(*cursor) {
        *cursor += 1;
        true
    } else {
        false
    };

    let left = logical_expression(cursor, tokens)?;

    if matches!(left, Expression::Literal { .. })
        || (!is_definition && matches!(left, Expression::Member { .. }))
    {
        if let Some(Token { kind: TokenKind::Assign | TokenKind::PlusEqual | TokenKind::MinusEqual | TokenKind::AsteriskEqual | TokenKind::SlashEqual | TokenKind::PercentEqual, .. }) = tokens.get(*cursor) {
            let operator = tokens.get(*cursor).unwrap().kind.clone();
            *cursor += 1;
            return Ok(Expression::Assignment {
                left: Box::new(left),
                operator,
                right: Box::new(logical_expression(cursor, tokens)?),
                is_definition,
            });
        }
    }

    Ok(left)
}

fn logical_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left: Expression = equality_expression(cursor, tokens)?;
    while matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::And | TokenKind::Or)) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        left = Expression::Logical {
            left: Box::new(left),
            operator,
            right: Box::new(equality_expression(cursor, tokens)?),
        };
    }

    Ok(left)
}

fn equality_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = relational_expression(cursor, tokens)?;
    while matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::EQ | TokenKind::NE)) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        left = Expression::Equality {
            left: Box::new(left),
            operator,
            right: Box::new(relational_expression(cursor, tokens)?),
        };
    }

    Ok(left)
}

fn relational_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = term_expression(cursor, tokens)?;
    while matches!(
        tokens.get(*cursor).map(|token| token.kind.clone()),
        Some(TokenKind::GT | TokenKind::GTE | TokenKind::LT | TokenKind::LTE)
    ) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        left = Expression::Relational {
            left: Box::new(left),
            operator,
            right: Box::new(term_expression(cursor, tokens)?),
        };
    }

    Ok(left)
}

fn term_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = factor_expression(cursor, tokens)?;
    while matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::Plus | TokenKind::Minus)) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(factor_expression(cursor, tokens)?),
        };
    }

    Ok(left)
}

fn factor_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let mut left = call_expression(cursor, tokens)?;
    while matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::Asterisk | TokenKind::Slash | TokenKind::Percent)) {
        let operator = tokens.get(*cursor).unwrap().clone().kind;
        *cursor += 1;

        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(call_expression(cursor, tokens)?),
        };
    }

    Ok(left)
}

fn call_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    if let Some(Token { kind: TokenKind::Identifier { value }, .. }) = tokens.get(*cursor) {
        if matches!(tokens.get(*cursor + 1).map(|token| token.kind.clone()), Some(TokenKind::OpeningParenthesis)) {
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

            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingParenthesis)) {
                panic!("Expected ')'");
            }

            *cursor += 1; // move past ')'

            return Ok(Expression::Call {
                callee: Box::new(Expression::Literal {
                    r#type: LiteralExpressionKind::Variable,
                    value: ValueHolder::String(name),
                }),
                arguments: args,
            });
        }
    }

    unary_expression(cursor, tokens)
}

fn unary_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    if matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::Plus) | Some(TokenKind::Minus)) {
        let token = tokens.get(*cursor).unwrap().clone();
        *cursor += 1;

        return Ok(Expression::Unary {
            left: Box::new(literal_expression(cursor, tokens)?),
            operator: token.kind
        })
    }

    literal_expression(cursor, tokens)
}

fn literal_expression(cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<Expression> {
    let token = tokens.get(*cursor).unwrap();

    *cursor += 1;

    let expr = match &token.kind {
        TokenKind::NumericLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Number(DynamicNumber::from_str(value)),
        },
        TokenKind::BooleanLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::Bool(*value),
        },
        TokenKind::StringLiteral { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Literal,
            value: ValueHolder::String(value.clone()),
        },
        TokenKind::Identifier { value } => Expression::Literal {
            r#type: LiteralExpressionKind::Variable,
            value: ValueHolder::String(value.clone()),
        },
        TokenKind::OpeningParenthesis => {
            let expr = left(cursor, tokens)?;

            tokens.get(*cursor).map_or_else(
                || panic!(") expected"),
                |token| matches!(token.kind, TokenKind::ClosingParenthesis),
            );

            *cursor += 1;

            expr
        }
        TokenKind::OpeningBracket => {
            let mut entries: HashMap<String, Expression> = HashMap::new();
            while !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingBracket)) {
                let key = literal_expression(cursor, tokens)?;

                let Expression::Literal {
                    value: ValueHolder::String(key),
                    ..
                } = key
                else {
                    todo!();
                };

                let Some(Token { kind: TokenKind::Colon, .. }) = tokens.get(*cursor) else {
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

            Expression::Literal {
                r#type: LiteralExpressionKind::Object(entries),
                value: ValueHolder::Void,
            }
        }
        TokenKind::OpeningSquareBracket => {
            let mut items: Vec<Expression> = vec![];

            while !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingSquareBracket)) {
                let item: Expression = literal_expression(cursor, tokens)?;

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

            Expression::Literal { r#type: LiteralExpressionKind::Array(items), value: ValueHolder::Void }
        }
        _ => return Err(LanguageError::with_source(ParserError::UnexpectedToken("function argument".into()), token.start, token.end)),
    };

    member(cursor, tokens, expr, true)
}

fn member(
    cursor: &mut usize,
    tokens: &mut Vec<Token>,
    mut expr: Expression,
    match_call: bool,
) -> LanguageResult<Expression> {
    loop {
        if matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::Period)) {
            *cursor += 1;
            let Some(Token { kind: TokenKind::Identifier { value }, .. }) = tokens.get(*cursor).cloned() else {
                panic!()
            };

            *cursor += 1;

            expr = Expression::Member {
                object: Box::new(expr),
                property: Box::new(Expression::Literal {
                    r#type: LiteralExpressionKind::Literal,
                    value: ValueHolder::String(value.to_string()),
                }),
            };
        } else if matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::OpeningSquareBracket)) {
            *cursor += 1;

            expr = Expression::Member {
                object: Box::new(expr),
                property: Box::new(logical_expression(cursor, tokens)?),
            };

            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingSquareBracket)) {
                panic!()
            }

            *cursor += 1;
        } else {
            break;
        }

        // Check if call
        if match_call && matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::OpeningParenthesis)) {
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
                            _ => return Err(LanguageError::with_source(ParserError::UnexpectedToken("',' or ')'".into()), token.start, token.end))
                        }
                    }
                }
            }

            if !matches!(tokens.get(*cursor).map(|token| token.kind.clone()), Some(TokenKind::ClosingParenthesis)) {
                let Token { start, end, .. } = tokens.get(*cursor).unwrap().clone();
                return Err(LanguageError::with_source(ParserError::UnexpectedToken("',' or ')'".into()), start, end))
            }

            *cursor += 1; // move past ')'

            expr = Expression::Call {
                callee: Box::new(expr),
                arguments: args,
            };
        }
    }

    Ok(expr)
}
