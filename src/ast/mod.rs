#[derive(Debug, Clone)]
pub enum Expression {
    Id(IdentifierExpr),
    Assign(AssignmentExpr),
    Literal(LiteralExpr),
    Call(CallExpr),
    Binary(BinaryExpr),
    If(IfExpr),
}

#[derive(Debug, Clone)]
pub enum LiteralExpr {
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub struct IdentifierExpr {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AssignmentExpr {
    pub target: String,
    pub expr: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: String,
    pub arguments: Vec<Expression>,
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
    pub operator: BinaryOperator,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Option<Box<Expression>>,
}
