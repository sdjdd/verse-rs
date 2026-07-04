use std::collections::HashMap;

use crate::core::{ConstId, ConstValue};

#[derive(Clone, Default)]
pub struct ConstPool {
    map: HashMap<ConstValue, usize>,
    vec: Vec<ConstValue>,
}

impl ConstPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, value: ConstValue) -> ConstId {
        if let Some(id) = self.map.get(&value) {
            return ConstId(*id);
        }

        let id = self.vec.len();
        self.map.insert(value.clone(), id);
        self.vec.push(value);
        ConstId(id)
    }

    pub fn into_table(self) -> Vec<ConstValue> {
        self.vec
    }
}
