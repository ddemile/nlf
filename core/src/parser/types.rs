use std::{
    cell::RefCell, collections::HashMap, fmt::{self, Debug}, rc::Rc, str::FromStr
};

use nlf_shared::{SchemaStore, indexmap::IndexMap};
use serde::Serialize;

use crate::{
    analysis::Span, errors::{LanguageError, LanguageErrorTrait}, interpreter::{ClassDefinition, ModuleContext, Object, RuntimeError, Scope, prototypes::Method}, lexer::{KeywordKind, TokenKind}
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
    pub schema_store: Rc<RefCell<SchemaStore>>,
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
    Number(f64),
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
            Self::Number(a) => a > 0.0,
            Self::String(a) => a.len() > 0,
            Self::Bool(a) => a,
            Self::Array(_) => true,
            Self::Object(_) => true,
            Self::ClassDefinition(_) => true,
            Self::Fn(_) => true,
            _ => false, // different variants cannot be compared
        }
    }
}

impl Into<i32> for ValueHolder {
    fn into(self) -> i32 {
        match self {
            ValueHolder::Number(a) => a as i32,
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
pub enum Iterable {
    Range(Option<Expression>, Option<Expression>),
    Array(Expression)
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
        iterable: Iterable,
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
    Continue,
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
    type Argument = Argument<Identifier, TypeId>;
    type Descriptor = VariableDescriptor;
    type Type = TypeId;
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

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableRef {
    pub name: Option<String>,
    pub slot: usize,
    pub depth: usize,
    pub start: usize,
    pub end: usize,
    pub upvalue: bool
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

#[derive(Serialize, Debug, Clone)]
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
    Named(Identifier),
    Array {
        type_ref: Box<TypeRef>,
        span: Span
    },
    Object {
        entries: IndexMap<String, TypeRef>,
        span: Span
    }
}

impl TypeRef {
    pub fn get_span(&self) -> Span {
        match self {
            Self::Named(ident) => Span { start: ident.start, end: ident.end },
            Self::Array { span, .. } | Self::Object { span, .. } => *span,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct FunctionType {
    pub arguments: Vec<Argument<Identifier, TypeId>>,
    pub return_ty: TypeId
}

#[derive(Serialize, Debug, Clone)]
pub struct FieldType {
    pub name: String,
    pub visibility: Visibility,
    pub ty: TypeId
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
    Array(Box<Type>),
    Object(IndexMap<String, Type>),
    Unknown,
    Any
}

impl ToString for Type {
    fn to_string(&self) -> String {
        match self {
            Self::String => "string".to_string(),
            Self::Number => "number".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Function(_function_type) => {
                format!("({}) => {}",
                    // function_type.arguments
                    //     .iter()
                    //     .map(|argument| format!("{}: {}", argument.variable.value, argument.ty.to_string()))
                    //     .collect::<Vec<String>>()
                    //     .join(", "),
                    // function_type.return_ty.to_string()
                    "not implemened",
                    "still not implemented"
                )
            },
            Self::Class(box ClassType { name, .. }) => format!("class {}", name),
            Self::Instance { class: box ClassType { name, .. }, .. } => format!("{}", name),
            Self::Array(ty) => format!("{}[]", ty.to_string()),
            Self::Object(entries) => {
                format!("{{ {} }}", entries.iter().map(|(key, value)| format!("{}: {}", key, value.to_string())).collect::<Vec<String>>().join(", "))
            },
            Self::Unknown => "unknown".to_string(),
            Self::Any => "any".to_string()
        }
    }
}

impl FromStr for DefaultType {
    type Err = LanguageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(DefaultType::String),
            "number" => Ok(DefaultType::Number),
            "bool" => Ok(DefaultType::Bool),
            "unknown" => Ok(DefaultType::Unknown),
            "any" => Ok(DefaultType::Any),
            _ => Err(LanguageError::from(RuntimeError::Custom("Failed to convert str to DefaultType".to_string())))
        }
    }
}

#[derive(Debug)]
pub struct TypeArena<T = Type> where T: Debug {
    types: Vec<T>,
    defaults: HashMap<DefaultType, TypeId>
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub enum DefaultType {
    String,
    Number,
    Bool,
    Any,
    Unknown
}

impl TypeArena<Type> {
    pub fn register_defaults(&mut self) {
        let mut register = |default_ty: DefaultType, ty: Type| {
            let type_id = self.alloc(ty);
            self.defaults.insert(default_ty, type_id);
        };

        register(DefaultType::String, Type::String);
        register(DefaultType::Number, Type::Number);
        register(DefaultType::Bool, Type::Bool);
        register(DefaultType::Any, Type::Any);
        register(DefaultType::Unknown, Type::Unknown);
    }

    pub fn get_default(&self, ty: DefaultType) -> TypeId {
        self.defaults.get(&ty).unwrap().clone()
    }
}

impl<T: Debug> TypeArena<T> {
    pub fn new() -> Self {
        Self { types: vec![], defaults: HashMap::new() }
    }

    pub fn alloc(&mut self, ty: T) -> TypeId {
        let id = self.types.len() as u32;
        self.types.push(ty);
        TypeId(id)
    }

    pub fn get(&self, id: TypeId) -> &T {
        &self.types[id.0 as usize]
    }
}