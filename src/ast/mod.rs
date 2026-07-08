use derive_more::{Constructor, From};

use crate::{
    core::{ConstId, Symbol},
    lexer::Span,
};

#[derive(Debug, Clone)]
pub struct Expression {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, From)]
pub enum ExprKind {
    Id(IdExpr),
    Decl(DeclExpr),
    VarDecl(VarDeclExpr),
    Set(SetExpr),
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(ConstId),
    Logic(bool),
    Call(CallExpr),
    Binary(BinaryExpr),
    If(IfExpr),
    Template(TemplateExpression),
    CompareChain(CompareChainExpr),
    Tuple(TupleExpr),
    Block(BlockExpr),
    Func(FunctionExpr),
    Type(TypeExpr),
    Member(MemberExpr),
    Construct(ConstructExpr),
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    Named(Symbol),
    Option(Box<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    Function {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
    Type,
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub span: Span,
    pub kind: TypeExprKind,
}

#[derive(Debug, Clone, Constructor)]
pub struct IdExpr {
    pub symbol: Symbol,
}

#[derive(Debug, Clone)]
pub enum LValueKind {
    Id(IdExpr),
}

#[derive(Debug, Clone)]
pub struct LValue {
    pub kind: LValueKind,
    pub span: Span,
}

impl TryFrom<Expression> for LValue {
    type Error = Expression;

    fn try_from(value: Expression) -> Result<Self, Self::Error> {
        match value.kind {
            ExprKind::Id(id) => Ok(Self {
                kind: LValueKind::Id(id),
                span: value.span,
            }),
            _ => Err(value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeclExpr {
    pub target: Symbol,
    pub typ: Option<TypeExpr>,
    pub value: Box<Expression>,
}

impl DeclExpr {
    pub fn new(target: Symbol, typ: Option<TypeExpr>, value: Expression) -> Self {
        Self {
            target,
            typ,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetExpr {
    pub target: LValue,
    pub expr: Box<Expression>,
}

impl SetExpr {
    pub fn new(target: LValue, expr: Expression) -> Self {
        Self {
            target,
            expr: expr.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarDeclExpr {
    pub name: IdExpr,
    pub typ: TypeExpr,
    pub expr: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}

impl CallExpr {
    pub fn new(callee: Expression, args: Vec<Expression>) -> Self {
        Self {
            callee: callee.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOperator {
    Plus,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOperator,
    pub lhs: Box<Expression>,
    pub rhs: Box<Expression>,
}

impl BinaryExpr {
    pub fn new(lhs: Expression, op: BinaryOperator, rhs: Expression) -> Self {
        Self {
            op,
            lhs: lhs.into(),
            rhs: rhs.into(),
        }
    }
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

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: Symbol,
    pub typ: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub name: Symbol,
    pub params: Vec<FunctionParam>,
    pub return_type: TypeExpr,
    pub body: Box<Expression>,
}

impl FunctionExpr {
    pub fn new(
        name: Symbol,
        params: Vec<FunctionParam>,
        return_type: TypeExpr,
        body: Expression,
    ) -> Self {
        Self {
            name,
            params,
            return_type,
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expression>,
    pub property: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct ConstructExpr {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}
