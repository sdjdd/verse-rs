use derive_more::{Constructor, From};

use crate::{core::Symbol, lexer::Span};

#[derive(Debug, Clone)]
pub struct Expression {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, From)]
pub enum ExprKind {
    Id(IdentifierExpr),
    Decl(DeclarationExpr),
    VarDecl(VarDeclExpr),
    Set(SetExpr),
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    Logic(bool),
    Call(CallExpr),
    Binary(BinaryExpr),
    If(IfExpr),
    Template(TemplateExpression),
    CompareChain(CompareChainExpr),
    Tuple(TupleExpr),
    Block(BlockExpr),
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    Named(Symbol),
    Generic { base: Symbol, args: Vec<TypeExpr> },
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, Constructor)]
pub struct IdentifierExpr {
    pub symbol: Symbol,
}

#[derive(Debug, Clone)]
pub enum LValueKind {
    Id(IdentifierExpr),
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
pub struct DeclarationExpr {
    pub target: LValue,
    pub typ: Option<TypeExpr>,
    pub value: Box<Expression>,
}

impl DeclarationExpr {
    pub fn new(target: LValue, typ: Option<TypeExpr>, value: Expression) -> Self {
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
    pub name: IdentifierExpr,
    pub typ: TypeExpr,
    pub expr: Box<Expression>,
}

impl VarDeclExpr {
    pub fn new(name: IdentifierExpr, typ: TypeExpr, expr: Expression) -> Self {
        Self {
            name,
            typ,
            expr: expr.into(),
        }
    }
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

#[derive(Debug, Clone)]
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
    Raw(String),
    Expr(Expression),
}

#[derive(Debug, Clone, Constructor)]
pub struct TemplateExpression {
    pub elements: Vec<TemplateElement>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Constructor)]
pub struct TupleExpr {
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone, Constructor)]
pub struct BlockExpr {
    pub body: Vec<Expression>,
}
