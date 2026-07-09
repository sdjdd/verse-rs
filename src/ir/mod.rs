use crate::{
    ast::{BinaryOp, CompareOp},
    core::ConstId,
    runtime::TypeId,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Slot(pub usize);

#[derive(Debug, Clone)]
pub struct Ir {
    pub kind: ExprKind,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Nop,
    LoadGlobal { slot: Slot },
    StoreGlobal { slot: Slot, value: Box<Ir> },
    LoadLocal { slot: Slot },
    StoreLocal { slot: Slot, value: Box<Ir> },
    LoadUpvalue { index: usize },
    StoreUpvalue { index: usize, value: Box<Ir> },
    Int(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(ConstId),
    Logic(bool),
    Option(Option<Box<Ir>>),
    Call(CallExpr),
    IndexTuple { tuple: Box<Ir>, index: usize },
    Add((Box<Ir>, Box<Ir>)),
    Sub((Box<Ir>, Box<Ir>)),
    Mul((Box<Ir>, Box<Ir>)),
    Div((Box<Ir>, Box<Ir>)),
    Neg(Box<Ir>),
    Not(Box<Ir>),
    If(IfExpr),
    Loop(Box<Ir>),
    Break,
    Template(Vec<TemplateElement>),
    CompareChain(CompareChainExpr),
    Tuple(Vec<Ir>),
    Block(Vec<Ir>),
    Func(FunctionExpr),
    Type(TypeId),

    Cast { ty: TypeId, value: Box<Ir> },
    GetLength(Box<Ir>),
}

#[derive(Debug, Clone)]
pub struct StoreLocalIr {
    pub slot: Slot,
    pub value: Box<Ir>,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Ir>,
    pub args: Vec<Ir>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub lhs: Box<Ir>,
    pub op: BinaryOp,
    pub rhs: Box<Ir>,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub test: Box<Ir>,
    pub then: Box<Ir>,
    pub alt: Option<Box<Ir>>,
}

#[derive(Debug, Clone)]
pub enum TemplateElement {
    String(ConstId),
    Expr(Box<Ir>),
}

#[derive(Debug, Clone)]
pub struct CompareChainExpr {
    pub head: Box<Ir>,
    pub rest: Vec<(CompareOp, Ir)>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum UpvalueDesc {
    Local(Slot),
    Upvalue(usize),
}

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub slot: Slot,
    pub params: Vec<Slot>,
    pub body: Box<Ir>,
    pub return_void: bool,
    pub upvalues: Vec<UpvalueDesc>,
}
