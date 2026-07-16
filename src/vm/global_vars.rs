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
    ps: PredefinedSymbols,
    pt: PredefinedTypes,
    type_reg: &mut TypeRegistry,
    get_symbol_slot: impl Fn(Symbol) -> usize,
) {
    let global_vars = [
        (ps.s_int, Value::Type(pt.t_int)),
        (ps.s_float, Value::Type(pt.t_float)),
        (ps.s_logic, Value::Type(pt.t_logic)),
        (ps.s_char, Value::Type(pt.t_char)),
        (ps.s_char32, Value::Type(pt.t_char32)),
        (ps.s_string, Value::Type(pt.t_string)),
        (ps.s_any, Value::Type(pt.t_any)),
        (ps.s_void, Value::Type(pt.t_void)),
        (
            ps.s_Print,
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
