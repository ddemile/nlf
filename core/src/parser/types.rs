use std::{
    cell::RefCell, collections::HashMap, fmt::{self, Debug}, rc::Rc, str::FromStr
};

use indexmap::IndexMap;
use serde::Serialize;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};

use crate::{
    analysis::Span, errors::{LanguageError, LanguageErrorTrait}, interpreter::{ClassDefinition, ModuleContext, Object, ProgramContext, RuntimeError, Scope, prototypes::Method}, lexer::{KeywordKind, TokenKind}
};

#[derive(Debug)]
pub enum ParserError {
    UnexpectedToken(String),
    InvalidType(String),
    UnexpectedEndOfInput,
    InvalidUsage(String)
}

impl LanguageErrorTrait for ParserError {}

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
    pub statements: Rc<[IRStatement]>,
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
pub struct Block<T> {
    pub statements: Rc<[T]>,
    pub start: usize,
    pub end: usize
}

pub type ASTBlock = Block<ASTStatement>;
pub type IRBlock = Block<IRStatement>;
pub type TypedBlock = Block<TypedStatement>;

impl<T> Block<T> {
    pub fn get_span(&self) -> Span {
        Span { start: self.start, end: self.end }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportSpecifier {
    pub local: Expression,
}

#[derive(Serialize, Debug, Clone)]
pub struct StringLiteral {
    pub value: String,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
pub struct Identifier {
    pub value: String,
    pub start: usize,
    pub end: usize
}

#[derive(Serialize, Debug, Clone)]
pub enum VariableDescriptor {
    Identifier(Identifier),
    Object(Vec<VariableDescriptor>)
}

#[derive(Serialize, Debug, Clone)]
pub struct Argument<T, TypeFormat> {
    pub variable: T,
    pub type_ref: Option<TypeRef>,
    pub ty: TypeFormat
}

#[derive(Serialize, Debug, Clone)]
pub struct Statement<T> {
    pub kind: T,
    pub start: usize,
    pub end: usize,
}

pub type ASTStatement = Statement<ASTStatementKind>;
pub type IRStatement = Statement<IRStatementKind>;
pub type TypedStatement = Statement<TypedStatementKind>;

pub trait SyntaxTree: Clone {
    type Variable: Serialize + Clone;
    type Descriptor: Serialize + Clone;
    type Type: Serialize + Clone;
    type Argument: Serialize + Clone;
    
    fn wrap_statement_kind_wrapper(statement_kind: StatementKind<Self>) -> StatementKindWrapper;

    fn unwrap_statement_kind_wrapper(statement_kind_wrapper: StatementKindWrapper) -> StatementKind<Self>;
}

#[derive(Serialize, Debug, Clone)]
pub enum StatementKind<A: SyntaxTree> {
    Expression {
        expression: Expression,
    },
    If {
        condition: Expression,
        block: Block<Statement<Self>>,
        alternate: Option<Box<Statement<Self>>>,
    },
    For {
        variable: A::Variable,
        left: Option<Expression>,
        right: Option<Expression>,
        statements: Rc<[Statement<Self>]>,
    },
    Function {
        variable: A::Variable,
        arguments: Vec<A::Argument>,
        block: Block<Statement<Self>>,
        return_type_ref: Option<TypeRef>,
        return_ty: A::Type
    },
    While {
        condition: Expression,
        statements: Rc<[Statement<Self>]>,
    },
    Return {
        expression: Expression,
    },
    Break,
    Block(Block<Statement<Self>>),
    Import {
        specifiers: Vec<A::Variable>,
        source: StringLiteral,
    },
    Export {
        declaration: Box<Statement<Self>>,
    },
    Class {
        variable: A::Variable,
        methods: Vec<Statement<Self>>,
        fields: Vec<ASTStatement>,
        body_start: usize,
        body_end: usize
    },
    Field {
        visibility: Visibility,
        name: String,
        value: Expression,
    },
    Method {
        name: String,
        arguments: Vec<VariableRef>,
        block: Block<IRStatement>
    },
    VariableDefinition {
        descriptor: A::Descriptor,
        expression: Expression,
        type_ref: Option<TypeRef>,
        ty: A::Type
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ASTSyntaxTree;

impl SyntaxTree for ASTSyntaxTree {
    type Argument = Argument<Self::Variable, Self::Type>;
    type Descriptor = VariableDescriptor;
    type Type = ();
    type Variable = Identifier;

    fn wrap_statement_kind_wrapper(statement_kind: StatementKind<Self>) -> StatementKindWrapper {
        StatementKindWrapper::AST(statement_kind)
    }

    fn unwrap_statement_kind_wrapper(statement_kind_wrapper: StatementKindWrapper) -> StatementKind<Self> {
        let StatementKindWrapper::AST(statement_kind) = statement_kind_wrapper else {
            unreachable!()
        };

        statement_kind
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IRSyntaxTree;

impl SyntaxTree for IRSyntaxTree {
    type Argument = Self::Variable;
    type Descriptor = Vec<Self::Variable>;
    type Type = ();
    type Variable = VariableRef;

    fn wrap_statement_kind_wrapper(statement_kind: StatementKind<Self>) -> StatementKindWrapper {
        StatementKindWrapper::IR(statement_kind)
    }

    fn unwrap_statement_kind_wrapper(statement_kind_wrapper: StatementKindWrapper) -> StatementKind<Self> {
        let StatementKindWrapper::IR(statement_kind) = statement_kind_wrapper else {
            unreachable!()
        };

        statement_kind
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TypedSyntaxTree;

impl SyntaxTree for TypedSyntaxTree {
    type Argument = Argument<Identifier, Self::Type>;
    type Descriptor = VariableDescriptor;
    type Type = Type;
    type Variable = Identifier;

    fn wrap_statement_kind_wrapper(statement_kind: StatementKind<Self>) -> StatementKindWrapper {
        StatementKindWrapper::Typed(statement_kind)
    }

    fn unwrap_statement_kind_wrapper(statement_kind_wrapper: StatementKindWrapper) -> StatementKind<Self> {
        let StatementKindWrapper::Typed(statement_kind) = statement_kind_wrapper else {
            unreachable!()
        };

        statement_kind
    }
}

pub type ASTStatementKind = StatementKind<ASTSyntaxTree>;
pub type IRStatementKind = StatementKind<IRSyntaxTree>; 
pub type TypedStatementKind = StatementKind<TypedSyntaxTree>; 

impl<A: SyntaxTree> StatementKind<A> {
    pub fn into_statement(self, start: usize, end: usize) -> Statement<Self> {
        Statement { kind: self, start, end }
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum StatementKindWrapper {
    AST(ASTStatementKind),
    Typed(TypedStatementKind),
    IR(IRStatementKind)
}

#[derive(Serialize, Debug, Clone)]
pub enum LiteralExpressionKind {
    Literal,
    Variable,
    Object(IndexMap<String, Expression>),
    Array(Vec<Expression>),
    Function(Box<StatementKindWrapper>)
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
        right: Box<Expression>
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
pub struct Program<T> {
    pub body: Rc<[T]>,
}

pub type ASTProgram = Program<ASTStatement>;
pub type IRProgram = Program<IRStatement>;
pub type TypedProgram = Program<TypedStatement>;

// Typing
#[derive(Serialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Serialize, Debug, Clone)]
pub enum TypeRef {
    Named(Identifier)
}

impl TypeRef {
    pub fn get_span(&self) -> Span {
        match self {
            Self::Named(ident) => Span { start: ident.start, end: ident.end }
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct FunctionType {
    pub arguments: Vec<Argument<Identifier, Type>>,
    pub return_ty: Type
}

#[derive(Serialize, Debug, Clone)]
pub struct FieldType {
    pub name: String,
    pub visibility: Visibility,
    pub ty: Type
}

#[derive(Serialize, Debug, Clone)]
pub struct ClassType {
    pub name: String,
    pub methods: HashMap<String, FunctionType>,
    pub fields: Vec<FieldType>
}

#[derive(Serialize, Debug, Clone)]
pub enum Type {
    String,
    Number,
    Bool,
    Function(Box<FunctionType>),
    Class(Box<ClassType>),
    Instance {
        class: Box<ClassType>
    },
    Unknown,
    Any
}

impl ToString for Type {
    fn to_string(&self) -> String {
        match self {
            Self::String => "string".to_string(),
            Self::Number => "number".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Function(function_type) => {
                format!("({}) => {}",
                    function_type.arguments
                        .iter()
                        .map(|argument| format!("{}: {}", argument.variable.value, argument.ty.to_string()))
                        .collect::<Vec<String>>()
                        .join(", "),
                    function_type.return_ty.to_string()
                )
            },
            Self::Class(box ClassType { name, .. }) => format!("class {}", name),
            Self::Instance { class: box ClassType { name, .. }, .. } => format!("{}", name),
            Self::Unknown => "unknown".to_string(),
            Self::Any => "any".to_string()
        }
    }
}

impl FromStr for Type {
    type Err = LanguageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(Type::String),
            "number" => Ok(Type::Number),
            "bool" => Ok(Type::Bool),
            "unknown" => Ok(Type::Unknown),
            "any" => Ok(Type::Any),
            _ => Err(LanguageError::from(RuntimeError::Custom("Failed to convert str to Type".to_string())))
        }
    }
}

pub struct TypeArena {
    types: Vec<Type>
}

impl TypeArena {
    pub fn new() -> Self {
        Self { types: vec![] }
    }

    pub fn alloc(&mut self, ty: Type) -> TypeId {
        let id = self.types.len() as u32;
        self.types.push(ty);
        TypeId(id)
    }

    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }
}