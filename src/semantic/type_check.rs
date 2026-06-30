use std::collections::HashMap;

use crate::core::Symbol;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct TypeId(usize);

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub enum TypeInfo {
    Incomplete,
    Void,
    Any,
    Int,
    Float,
    Logic,
    Char,
    Char32,
    String,
    Tuple(Vec<TypeId>),
    Function { params: Vec<TypeId>, ret: TypeId },
    Option(TypeId),
    Named(Symbol),
}

#[derive(Default, Debug)]
pub struct TypeRegistry {
    map: HashMap<TypeInfo, TypeId>,
    vec: Vec<TypeInfo>,
    alias: HashMap<TypeId, TypeId>,
}

impl TypeRegistry {
    pub fn intern(&mut self, key: TypeInfo) -> TypeId {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }

        let key = match &key {
            TypeInfo::Tuple(element_ids) => {
                TypeInfo::Tuple(element_ids.iter().map(|&id| self.resolve_id(id)).collect())
            }
            _ => key,
        };

        let id = TypeId(self.vec.len());
        self.map.insert(key.clone(), id);
        self.vec.push(key);
        id
    }

    pub fn set_alias(&mut self, src: TypeId, dst: TypeId) {
        self.alias.insert(dst, src);
    }

    pub fn resolve_id(&self, mut id: TypeId) -> TypeId {
        loop {
            if let Some(&src_id) = self.alias.get(&id) {
                id = src_id
            } else {
                break id;
            }
        }
    }

    pub fn resolve(&self, type_info: &TypeInfo) -> Option<TypeId> {
        if let Some(&id) = self.map.get(type_info) {
            Some(self.resolve_id(id))
        } else {
            None
        }
    }

    pub fn lookup(&self, type_id: TypeId) -> Option<&TypeInfo> {
        self.vec.get(type_id.0)
    }
}
