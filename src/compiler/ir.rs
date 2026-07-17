use crate::compiler::ast::{BinaryOp, CompareOp};
use crate::core::ConstId;
use crate::core::types::TypeInfo;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Slot(pub usize);

#[derive(Debug, Clone)]
pub struct Ir {
    pub kind: ExprKind,
    pub ty: TypeInfo,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
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
    Tuple(Vec<Ir>),
    IndexTuple { tuple: Box<Ir>, index: usize },
    Array(Vec<Ir>),
    Call(CallExpr),
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
    Block(Vec<Ir>),
    Func(FunctionExpr),
    Type(TypeInfo),

    Cast { ty: TypeInfo, value: Box<Ir> },
    GetLength(Box<Ir>),
    Concat(Vec<Ir>),
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

#[derive(Debug, Clone, Copy)]
pub struct Effects {
    pub decides: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub slot: Slot,
    pub params: Vec<Slot>,
    pub effects: Effects,
    pub body: Box<Ir>,
    pub return_void: bool,
    pub upvalues: Vec<UpvalueDesc>,
}
