use crate::{
    core::{Symbol, SymbolTable},
    semantic::{TypeId, TypeInfo, TypeRegistry},
};

pub struct BuiltinSymbols {
    // types
    pub(crate) s_int: Symbol,
    pub(crate) s_float: Symbol,
    pub(crate) s_char: Symbol,
    pub(crate) s_char32: Symbol,
    pub(crate) s_logic: Symbol,
    pub(crate) s_string: Symbol,
    pub(crate) s_void: Symbol,
    pub(crate) s_any: Symbol,

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
        let s_void = symbol_tbl.intern("void");
        let s_any = symbol_tbl.intern("any");

        let s_print = symbol_tbl.intern("Print");

        Self {
            s_int,
            s_float,
            s_char,
            s_char32,
            s_logic,
            s_string,
            s_void,
            s_print,
            s_any,
        }
    }
}

pub struct BuiltinTypes {
    pub(crate) t_int: TypeId,
    pub(crate) t_float: TypeId,
    pub(crate) t_logic: TypeId,
    pub(crate) t_char: TypeId,
    pub(crate) t_char32: TypeId,
    pub(crate) t_string: TypeId,
    pub(crate) t_any: TypeId,
    pub(crate) t_void: TypeId,
}

impl BuiltinTypes {
    pub fn install(type_reg: &mut TypeRegistry) -> Self {
        let t_int = type_reg.intern(TypeInfo::Int);
        let t_float = type_reg.intern(TypeInfo::Float);
        let t_logic = type_reg.intern(TypeInfo::Logic);
        let t_char = type_reg.intern(TypeInfo::Char);
        let t_char32 = type_reg.intern(TypeInfo::Char32);
        let t_string = type_reg.intern(TypeInfo::String);
        let t_any = type_reg.intern(TypeInfo::Any);
        let t_void = type_reg.intern(TypeInfo::Void);

        Self {
            t_int,
            t_float,
            t_logic,
            t_char,
            t_char32,
            t_string,
            t_any,
            t_void,
        }
    }

    pub fn pairs(&self, bs: &BuiltinSymbols) -> Vec<(Symbol, TypeId)> {
        vec![
            (bs.s_int, self.t_int),
            (bs.s_float, self.t_float),
            (bs.s_logic, self.t_logic),
            (bs.s_char, self.t_char),
            (bs.s_char32, self.t_char32),
            (bs.s_string, self.t_string),
            (bs.s_any, self.t_any),
            (bs.s_void, self.t_void),
        ]
    }
}
