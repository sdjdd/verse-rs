use derive_more::{Constructor, Display, From};
use derive_new::new;

use super::lexer::Span;
use crate::core::{ConstId, Symbol};

#[derive(Debug, Clone, new)]
pub struct Expression {
    pub id: u32,
    pub span: Span,
    #[new(into)]
    pub kind: ExprKind,
}

#[derive(Debug, Clone, From)]
pub enum ExprKind {
    Id(IdExpr),
    Decl(DeclExpr),
    Init(InitExpr),
    Set(SetExpr),
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(ConstId),
    Logic(bool),
    Call(CallExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    If(IfExpr),
    Loop(Box<Expression>),
    Break,
    Template(TemplateExpression),
    CompareChain(CompareChainExpr),
    Tuple(TupleExpr),
    Block(BlockExpr),
    Func(FunctionExpr),
    Type(TypeExpr),
    Member(MemberExpr),
    Construct(ConstructExpr),
    Query(QueryExpr),
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    Named(Symbol),
    Option(Box<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    Array(Box<TypeExpr>),
    Struct(Vec<StructField>),
    Class(Vec<ClassMember>),
    Function {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
    Type,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: IdExpr,
    pub ty: TypeExpr,
    pub default: Expression,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Var {
        name: IdExpr,
        ty: TypeExpr,
        default: Option<Expression>,
        mutable: bool,
    },
    Method(FunctionExpr),
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub span: Span,
    pub kind: TypeExprKind,
}

#[derive(Debug, Clone, Constructor)]
pub struct IdExpr {
    pub id: u32,
    pub span: Span,
    pub symbol: Symbol,
}

impl Into<Expression> for IdExpr {
    fn into(self) -> Expression {
        Expression {
            id: self.id,
            span: self.span.clone(),
            kind: ExprKind::Id(self),
        }
    }
}

#[derive(Debug, Clone, new)]
pub struct DeclExpr {
    pub id: u32,
    pub span: Span,
    pub name: IdExpr,
    pub ty: TypeExpr,
    #[new(into)]
    pub value: Box<Expression>,
    pub mutable: bool,
}

impl Into<Expression> for DeclExpr {
    fn into(self) -> Expression {
        Expression {
            id: self.id,
            span: self.span.clone(),
            kind: ExprKind::Decl(self),
        }
    }
}

#[derive(Debug, Clone, new)]
pub struct InitExpr {
    pub name: IdExpr,
    #[new(into)]
    pub value: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct SetExpr {
    pub lhs: Box<Expression>,
    pub rhs: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
    pub fallible: bool,
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    #[display("add")]
    Add,
    #[display("subtract")]
    Sub,
    #[display("multiply")]
    Mul,
    #[display("divide")]
    Div,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub op_span: Span,
    pub lhs: Box<Expression>,
    pub rhs: Box<Expression>,
}

#[derive(Debug, Clone, Copy, Display)]
pub enum UnaryOp {
    #[display("+")]
    Plus,
    #[display("-")]
    Minus,
    #[display("not")]
    Not,
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Option<Box<Expression>>,
}

impl IfExpr {
    pub fn new(test: Expression, consequent: Expression, alternate: Option<Expression>) -> Self {
        Self {
            test: Box::new(test),
            consequent: Box::new(consequent),
            alternate: alternate.map(Box::new),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TemplateElement {
    Raw(ConstId),
    Expr(Expression),
}

#[derive(Debug, Clone, Constructor)]
pub struct TemplateExpression {
    pub elements: Vec<TemplateElement>,
}

#[derive(Debug, Clone, Copy)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Debug, Clone)]
pub struct CompareChainExpr {
    pub head: Box<Expression>,
    pub rest: Vec<(CompareOp, Expression)>,
}

impl CompareChainExpr {
    pub fn new(head: Expression, rest: Vec<(CompareOp, Expression)>) -> Self {
        Self {
            head: head.into(),
            rest,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TupleExpr {
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone, Constructor)]
pub struct BlockExpr {
    pub body: Vec<Expression>,
}

#[derive(Debug, Clone, Constructor)]
pub struct FunctionParam {
    pub name: Symbol,
    pub typ: TypeExpr,
}

#[derive(Debug, Clone, new)]
pub struct FunctionExpr {
    pub id: u32,
    pub span: Span,
    pub name: Symbol,
    pub params: Vec<FunctionParam>,
    pub effects: Vec<IdExpr>,
    pub return_type: TypeExpr,
    #[new(into)]
    pub body: Box<Expression>,
}

impl Into<Expression> for FunctionExpr {
    fn into(self) -> Expression {
        Expression {
            id: self.id,
            span: self.span.clone(),
            kind: ExprKind::Func(self),
        }
    }
}

#[derive(Debug, Clone, new)]
pub struct MemberExpr {
    #[new(into)]
    pub object: Box<Expression>,
    #[new(into)]
    pub property: Box<IdExpr>,
}

#[derive(Debug, Clone, new)]
pub struct ConstructExpr {
    #[new(into)]
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}

#[derive(Debug, Clone, new)]
pub struct QueryExpr {
    #[new(into)]
    pub expr: Box<Expression>,
}
