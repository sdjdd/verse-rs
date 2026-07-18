use crate::compiler::ast::{BinaryOp, CompareOp};
use crate::core::ConstId;
use crate::core::types::TypeInfo;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Slot(pub usize);

#[derive(Debug, Clone)]
pub struct Ir {
    pub kind: IrKind,
    pub ty: TypeInfo,
}

#[derive(Debug, Clone)]
pub enum IrKind {
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
    IndexArray { array: Box<Ir>, index: Box<Ir> },
    Call(CallIr),
    Add((Box<Ir>, Box<Ir>)),
    Sub((Box<Ir>, Box<Ir>)),
    Mul((Box<Ir>, Box<Ir>)),
    Div((Box<Ir>, Box<Ir>)),
    Neg(Box<Ir>),
    Not(Box<Ir>),
    If(IfIr),
    Loop(Box<Ir>),
    Break,
    Template(Vec<TemplateElementIr>),
    CompareChain(CompareChainIr),
    Block(Vec<Ir>),
    Func(FunctionIr),
    Type(TypeInfo),

    Cast { ty: TypeInfo, value: Box<Ir> },
    GetLength(Box<Ir>),
    Concat(Vec<Ir>),
}

impl IrKind {
    pub fn is_fallible(&self) -> bool {
        match self {
            IrKind::Cast { .. }
            | IrKind::Div(_)
            | IrKind::CompareChain(_)
            | IrKind::IndexArray { .. } => true,

            IrKind::LoadLocal { .. }
            | IrKind::LoadGlobal { .. }
            | IrKind::LoadUpvalue { .. }
            | IrKind::Int(_)
            | IrKind::Float(_)
            | IrKind::Char(_)
            | IrKind::Char32(_)
            | IrKind::String(_)
            | IrKind::Logic(_)
            | IrKind::Break
            | IrKind::Type(_) => false,

            IrKind::StoreGlobal { value: ir, .. }
            | IrKind::StoreLocal { value: ir, .. }
            | IrKind::StoreUpvalue { value: ir, .. }
            | IrKind::Neg(ir)
            | IrKind::Not(ir)
            | IrKind::IndexTuple { tuple: ir, .. }
            | IrKind::Loop(ir)
            | IrKind::GetLength(ir) => ir.kind.is_fallible(),

            IrKind::Add((a, b)) | IrKind::Sub((a, b)) | IrKind::Mul((a, b)) => {
                a.kind.is_fallible() || b.kind.is_fallible()
            }

            IrKind::Tuple(irs) | IrKind::Array(irs) | IrKind::Block(irs) | IrKind::Concat(irs) => {
                irs.iter().any(|ir| ir.kind.is_fallible())
            }

            IrKind::Option(v) => v.as_ref().is_some_and(|ir| ir.kind.is_fallible()),
            IrKind::If(e) => {
                e.test.kind.is_fallible()
                    || e.then.kind.is_fallible()
                    || e.alt.as_ref().is_some_and(|e| e.kind.is_fallible())
            }

            IrKind::Func(func) => func.effects.decides,
            IrKind::Call(e) => e.callee.kind.is_fallible(),

            IrKind::Template(elems) => elems.iter().any(|e| match e {
                TemplateElementIr::Expr(ir) => ir.kind.is_fallible(),
                _ => false,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallIr {
    pub callee: Box<Ir>,
    pub args: Vec<Ir>,
}

#[derive(Debug, Clone)]
pub struct BinaryIr {
    pub lhs: Box<Ir>,
    pub op: BinaryOp,
    pub rhs: Box<Ir>,
}

#[derive(Debug, Clone)]
pub struct IfIr {
    pub test: Box<Ir>,
    pub then: Box<Ir>,
    pub alt: Option<Box<Ir>>,
}

#[derive(Debug, Clone)]
pub enum TemplateElementIr {
    String(ConstId),
    Expr(Box<Ir>),
}

#[derive(Debug, Clone)]
pub struct CompareChainIr {
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
pub struct FunctionIr {
    pub slot: Slot,
    pub params: Vec<Slot>,
    pub effects: Effects,
    pub body: Box<Ir>,
    pub return_void: bool,
    pub upvalues: Vec<UpvalueDesc>,
}
