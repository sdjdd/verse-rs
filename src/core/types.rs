use std::collections::HashMap;

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
}

#[derive(Default)]
pub struct TypeRegistry {
    map: HashMap<TypeInfo, TypeId>,
    vec: Vec<TypeInfo>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, key: TypeInfo) -> TypeId {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }

        let id = TypeId(self.vec.len());
        self.vec.push(key.clone());
        self.map.insert(key, id);
        id
    }

    pub fn lookup(&self, id: TypeId) -> Option<&TypeInfo> {
        self.vec.get(id.0)
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
