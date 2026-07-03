use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Symbol(pub(crate) usize);

#[derive(Clone, Default)]
pub struct SymbolTable {
    map: HashMap<String, usize>,
    vec: Vec<String>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&id) = self.map.get(name) {
            return Symbol(id);
        }

        let id = self.vec.len();
        self.map.insert(name.to_string(), id);
        self.vec.push(name.to_string());
        Symbol(id)
    }

    pub fn resolve(&self, symbol: Symbol) -> &str {
        self.vec.get(symbol.0).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstValue {
    String(String),
}

#[derive(Debug, Clone, Copy)]
pub struct ConstId(pub(crate) usize);

#[derive(Clone, Default)]
pub struct ConstTable {
    map: HashMap<ConstValue, usize>,
    vec: Vec<ConstValue>,
}

impl ConstTable {
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

    pub fn get(&self, id: ConstId) -> Option<&ConstValue> {
        self.vec.get(id.0)
    }
}
