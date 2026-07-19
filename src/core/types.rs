use std::fmt::{Display, Formatter};

use ordermap::OrderSet;

use crate::runtime::TypeId;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum TypeInfo {
    Void,
    Any,
    Int,
    Float,
    Rational,
    True,
    False,
    Logic,
    Char,
    Char32,
    String,

    Option(Box<TypeInfo>),
    Tuple(Vec<TypeInfo>),
    Array(Box<TypeInfo>),
    Function {
        params: Vec<TypeInfo>,
        ret: Box<TypeInfo>,
    },

    Type(Box<TypeInfo>),

    Bottom,

    Unknown,
}

impl TypeInfo {
    pub fn is_complete(&self) -> bool {
        match self {
            TypeInfo::Unknown => false,
            TypeInfo::Option(t) | TypeInfo::Array(t) | TypeInfo::Type(t) => t.is_complete(),
            TypeInfo::Tuple(ts) => ts.iter().all(|t| t.is_complete()),
            TypeInfo::Function { params, ret } => {
                params.iter().all(|t| t.is_complete()) && ret.is_complete()
            }
            _ => true,
        }
    }
}

impl Display for TypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeInfo::Void => write!(f, "void"),
            TypeInfo::Any => write!(f, "any"),
            TypeInfo::Int => write!(f, "int"),
            TypeInfo::Float => write!(f, "float"),
            TypeInfo::Rational => write!(f, "rational"),
            TypeInfo::True => write!(f, "true"),
            TypeInfo::False => write!(f, "false"),
            TypeInfo::Logic => write!(f, "logic"),
            TypeInfo::Char => write!(f, "char"),
            TypeInfo::Char32 => write!(f, "char32"),
            TypeInfo::String => write!(f, "string"),
            TypeInfo::Option(inner) => write!(f, "?{inner}"),
            TypeInfo::Tuple(elements) => {
                write!(f, "tuple(")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ")")
            }
            TypeInfo::Array(elem) => write!(f, "[]{elem}"),
            TypeInfo::Function { params, ret } => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, ":{param}")?;
                }
                write!(f, "): {ret}")
            }
            TypeInfo::Type(inner) => write!(f, "{inner}"),
            TypeInfo::Bottom => write!(f, "bottom"),
            TypeInfo::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Default)]
pub struct TypeRegistry {
    set: OrderSet<TypeInfo>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, key: TypeInfo) -> TypeId {
        let index = self.set.insert_full(key).0;
        TypeId(index as u32)
    }

    pub fn lookup(&self, id: TypeId) -> Option<&TypeInfo> {
        self.set.get_index(id.0 as usize)
    }
}

#[derive(Clone, Copy)]
pub struct PredefinedTypes {
    pub t_any: TypeId,
    pub t_bottom: TypeId,
    pub t_char: TypeId,
    pub t_char32: TypeId,
    pub t_false: TypeId,
    pub t_float: TypeId,
    pub t_int: TypeId,
    pub t_logic: TypeId,
    pub t_rational: TypeId,
    pub t_string: TypeId,
    pub t_true: TypeId,
    pub t_void: TypeId,
}

impl PredefinedTypes {
    pub fn install(reg: &mut TypeRegistry) -> Self {
        Self {
            t_any: reg.intern(TypeInfo::Any),
            t_bottom: reg.intern(TypeInfo::Bottom),
            t_char: reg.intern(TypeInfo::Char),
            t_char32: reg.intern(TypeInfo::Char32),
            t_false: reg.intern(TypeInfo::False),
            t_float: reg.intern(TypeInfo::Float),
            t_int: reg.intern(TypeInfo::Int),
            t_logic: reg.intern(TypeInfo::Logic),
            t_rational: reg.intern(TypeInfo::Rational),
            t_string: reg.intern(TypeInfo::String),
            t_true: reg.intern(TypeInfo::True),
            t_void: reg.intern(TypeInfo::Void),
        }
    }
}
