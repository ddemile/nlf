use std::{
    cell::RefCell, fmt::{self, Debug}, rc::Rc
};

use indexmap::IndexMap;
use serde::Serialize;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};

use crate::{
    errors::LanguageErrorTrait,
    interpreter::{ClassDefinition, ModuleContext, Object, ProgramContext, Scope, prototypes::Method},
    lexer::{KeywordKind, TokenKind},
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
    UnexpectedEndOfInput,
    InvalidUsage(String)
}

impl LanguageErrorTrait for ParserError {}

#[derive(Serialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ObjectRef {
    #[serde(skip)]
    pub object: Rc<RefCell<Object>>,
    #[serde(skip)]
    pub program_context: Rc<RefCell<ProgramContext>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ArrayRef {
    #[serde(skip)]
    pub object: Rc<RefCell<Object>>,
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
            (ValueHolder::Object(a), ValueHolder::Object(b)) => {
                let a = a.fetch();
                let b = b.fetch();

                a == b
            },
            (ValueHolder::Array(a), ValueHolder::Array(b)) => {
                let a = a.object.borrow();
                let b = b.object.borrow();

                a.values == b.values
            },
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
    pub fn into_statement(self, start: usize, end: usize) -> Statement {
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
    Object(IndexMap<String, Expression>),
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