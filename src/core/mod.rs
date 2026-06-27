use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Symbol(pub(crate) usize);

#[derive(Clone, Default)]
pub struct SymbolTable {
    map: HashMap<String, Symbol>,
    vec: Vec<String>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&id) = self.map.get(name) {
            return id;
        }

        let id = Symbol(self.vec.len());
        self.map.insert(name.to_string(), id);
        self.vec.push(name.to_string());
        id
    }

    pub fn resolve(&self, symbol: Symbol) -> &str {
        self.vec.get(symbol.0).unwrap()
    }
}
