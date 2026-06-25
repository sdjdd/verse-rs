use derive_more::{Constructor, From};

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub ln: usize,
    pub col: usize,
}

impl Default for Position {
    fn default() -> Self {
        Self { ln: 1, col: 1 }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.ln, self.col)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourceLoc {
    pub start: Position,
    pub end: Position,
}

impl std::fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

#[derive(Debug, Clone)]
pub struct Expression {
    pub loc: SourceLoc,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, From)]
pub enum ExprKind {
    Id(IdentifierExpr),
    Assign(AssignmentExpr),
    Literal(LiteralExpr),
    Call(CallExpr),
    Binary(BinaryExpr),
    If(IfExpr),
    Template(TemplateExpression),
    CompareChain(CompareChainExpr),
    Tuple(TupleExpr),
}

#[derive(Debug, Clone)]
pub enum LiteralExpr {
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    Bool(bool),
}

#[derive(Debug, Clone, Constructor)]
pub struct IdentifierExpr {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AssignmentExpr {
    pub target: String,
    pub expr: Box<Expression>,
}

impl AssignmentExpr {
    pub fn new(target: String, expr: Expression) -> Self {
        Self {
            target,
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
