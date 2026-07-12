use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Symbol(pub(crate) usize);

#[derive(Clone, Default)]
pub struct SymbolRegistry {
    map: HashMap<String, Symbol>,
    vec: Vec<String>,
}

impl SymbolRegistry {
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

    pub fn lookup(&self, symbol: Symbol) -> &str {
        self.vec.get(symbol.0).unwrap()
    }
}

#[allow(non_snake_case)]
#[derive(Clone, Copy)]
pub struct PredefinedSymbols {
    pub s_Length: Symbol,
    pub s_Print: Symbol,
    pub s_any: Symbol,
    pub s_array: Symbol,
    pub s_char: Symbol,
    pub s_char32: Symbol,
    pub s_float: Symbol,
    pub s_int: Symbol,
    pub s_logic: Symbol,
    pub s_option: Symbol,
    pub s_string: Symbol,
    pub s_tuple: Symbol,
    pub s_void: Symbol,
}

impl PredefinedSymbols {
    pub fn install(reg: &mut SymbolRegistry) -> Self {
        Self {
            s_Length: reg.intern("Length"),
            s_Print: reg.intern("Print"),
            s_any: reg.intern("any"),
            s_array: reg.intern("array"),
            s_char: reg.intern("char"),
            s_char32: reg.intern("char32"),
            s_float: reg.intern("float"),
            s_int: reg.intern("int"),
            s_logic: reg.intern("logic"),
            s_option: reg.intern("option"),
            s_string: reg.intern("string"),
            s_tuple: reg.intern("tuple"),
            s_void: reg.intern("void"),
        }
    }
}
