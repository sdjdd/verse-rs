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

#[derive(Debug, Clone, Copy)]
pub struct ConstId(pub(crate) usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstValue {
    String(String),
}
