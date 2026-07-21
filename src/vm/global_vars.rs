use std::collections::HashMap;

use crate::{
    core::{
        Symbol, SymbolRegistry,
        types::{TypeInfo, TypeRegistry},
    },
    runtime::{FnKind, Value, builtin_funcs, heap::Heap},
    vm::Vm,
};

pub fn install(
    vm: &mut Vm,
    symbol_table: &mut SymbolRegistry,
    type_registry: &mut TypeRegistry,
    symbol_slots: HashMap<Symbol, usize>,
) {
    let types = [
        ("int", TypeInfo::Int),
        ("float", TypeInfo::Float),
        ("logic", TypeInfo::Logic),
        ("char", TypeInfo::Char),
        ("char32", TypeInfo::Char32),
        ("string", TypeInfo::String),
        ("any", TypeInfo::Any),
        ("void", TypeInfo::Void),
    ];

    let functions = [(
        "Print",
        TypeInfo::Function {
            params: vec![TypeInfo::Any],
            ret: Box::new(TypeInfo::Void),
        },
        builtin_funcs::print,
    )];

    let mut global_vars = Vec::with_capacity(types.len() + functions.len());

    for (name, ty) in types {
        let symbol = symbol_table.intern(name);
        let type_id = type_registry.intern(ty);
        global_vars.push((symbol_slots[&symbol], Value::Type(type_id)));
    }

    for (name, ty, func) in functions {
        let symbol = symbol_table.intern(name);
        let type_id = type_registry.intern(ty);
        let obj_id = vm.heap.alloc_obj(Value::Function {
            type_id,
            kind: FnKind::Native(func),
        });
        global_vars.push((symbol_slots[&symbol], Value::Ref(obj_id)));
    }

    global_vars.sort_by(|a, b| a.0.cmp(&b.0));

    vm.stack.resize(global_vars.len(), Value::Void);

    for (i, v) in global_vars {
        vm.stack[i] = v;
    }
}
