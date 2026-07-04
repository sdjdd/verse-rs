use crate::{
    ast::{BinaryOperator, CompareOp},
    core::{ConstId, Symbol},
    runtime::TypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub usize);

#[derive(Debug, Clone)]
pub struct Ir {
    pub id: ExprId,
    pub kind: ExprKind,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Id(Symbol),
    Set(SetExpr),
    Int(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(ConstId),
    Logic(bool),
    Option(Option<ExprId>),
    Call(CallExpr),
    GetTupleElem { tuple: ExprId, index: usize },
    Binary(BinaryExpr),
    If(IfExpr),
    Template(Vec<TemplateElement>),
    CompareChain(CompareChainExpr),
    Tuple(Vec<ExprId>),
    Block(Vec<ExprId>),
    Func(FunctionExpr),
    Type(TypeId),

    Cast { ty: TypeId, value: ExprId },
    GetLength(ExprId),
}

#[derive(Debug, Clone)]
pub struct SetExpr {
    pub target: Symbol,
    pub value: ExprId,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: ExprId,
    pub args: Vec<ExprId>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub lhs: ExprId,
    pub op: BinaryOperator,
    pub rhs: ExprId,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub test: ExprId,
    pub then: ExprId,
    pub alt: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub enum TemplateElement {
    String(ConstId),
    Expr(ExprId),
}

#[derive(Debug, Clone)]
pub struct CompareChainExpr {
    pub head: ExprId,
    pub rest: Vec<(CompareOp, ExprId)>,
}

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub name: Symbol,
    pub params: Vec<Symbol>,
    pub body: ExprId,
    pub return_void: bool,
}
