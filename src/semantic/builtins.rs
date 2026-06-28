use crate::core::{Symbol, SymbolTable};

pub struct BuiltinSymbols {
    // types
    pub(crate) s_int: Symbol,
    pub(crate) s_float: Symbol,
    pub(crate) s_char: Symbol,
    pub(crate) s_char32: Symbol,
    pub(crate) s_logic: Symbol,
    pub(crate) s_string: Symbol,
    // functions
    pub(crate) s_print: Symbol,
}

impl BuiltinSymbols {
    pub fn install(symbol_tbl: &mut SymbolTable) -> Self {
        let s_int = symbol_tbl.intern("int");
        let s_float = symbol_tbl.intern("float");
        let s_char = symbol_tbl.intern("char");
        let s_char32 = symbol_tbl.intern("char32");
        let s_logic = symbol_tbl.intern("logic");
        let s_string = symbol_tbl.intern("string");

        let s_print = symbol_tbl.intern("Print");

        Self {
            s_int,
            s_float,
            s_char,
            s_char32,
            s_logic,
            s_string,
            s_print,
        }
    }
}
