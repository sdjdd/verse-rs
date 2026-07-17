use crate::{
    core::{
        PredefinedSymbols, Symbol,
        types::{PredefinedTypes, TypeInfo, TypeRegistry},
    },
    runtime::{FnKind, Value, builtin_funcs, heap::Heap},
    vm::Vm,
};

pub fn install(
    vm: &mut Vm,
    predefined_symbols: PredefinedSymbols,
    predefined_types: PredefinedTypes,
    type_reg: &mut TypeRegistry,
    get_symbol_slot: impl Fn(Symbol) -> usize,
) {
    let global_vars = [
        (predefined_symbols.s_int, Value::Type(predefined_types.t_int)),
        (predefined_symbols.s_float, Value::Type(predefined_types.t_float)),
        (predefined_symbols.s_logic, Value::Type(predefined_types.t_logic)),
        (predefined_symbols.s_char, Value::Type(predefined_types.t_char)),
        (predefined_symbols.s_char32, Value::Type(predefined_types.t_char32)),
        (predefined_symbols.s_string, Value::Type(predefined_types.t_string)),
        (predefined_symbols.s_any, Value::Type(predefined_types.t_any)),
        (predefined_symbols.s_void, Value::Type(predefined_types.t_void)),
        (
            predefined_symbols.s_Print,
            Value::Function {
                type_id: type_reg.intern(TypeInfo::Function {
                    params: vec![TypeInfo::Any],
                    ret: Box::new(TypeInfo::Void),
                }),
                kind: FnKind::Native(builtin_funcs::print),
            },
        ),
    ];

    let mut global_vars: Vec<_> = global_vars
        .into_iter()
        .map(|(s, v)| (get_symbol_slot(s), v))
        .collect();

    global_vars.sort_by(|a, b| a.0.cmp(&b.0));

    vm.stack.resize(global_vars.len(), Value::False);
    for (i, v) in global_vars {
        let v = match v {
            Value::Function { .. } => {
                let obj_id = vm.heap.alloc_obj(v);
                Value::Ref(obj_id)
            }
            v => v,
        };
        vm.stack[i] = v;
    }
}
