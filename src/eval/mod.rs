use crate::{
    compiler::ast::CompareOp,
    compiler::ir::{self, ExprKind, FunctionExpr, Ir, Slot, UpvalueDesc},
    compiler::semantic::Scope,
    core::{ConstValue, PredefinedSymbols, types::PredefinedTypes},
    runtime::{
        CallContext, Failure, FnKind, FunctionId, TypeId, Value,
        builtin_funcs::{self, write_value},
        heap::{Heap, ObjectId, SimpleHeap},
    },
};

#[derive(Default)]
struct CallFrame {
    base: usize,
    upvalues: Vec<ObjectId>,
}

pub struct Evaluator<THeap: Heap = SimpleHeap> {
    predefined_types: PredefinedTypes,
    const_table: Vec<ConstValue>,
    functions: Vec<FunctionExpr>,
    heap: THeap,
    stack: Vec<Value>,
    call_frames: Vec<CallFrame>,
    break_flag: bool,
}

impl Evaluator {
    pub fn new(
        ps: PredefinedSymbols,
        pt: PredefinedTypes,
        const_table: Vec<ConstValue>,
        root_scope: &Scope,
    ) -> Self {
        let global_bindings = [
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
                    kind: FnKind::Native(builtin_funcs::print),
                },
            ),
        ];

        let mut global_vars: Vec<_> = global_bindings
            .into_iter()
            .map(|(symbol, value)| (root_scope.lookup(symbol).unwrap().slot.0, value))
            .collect();

        global_vars.sort_by(|a, b| a.0.cmp(&b.0));
        let stack: Vec<_> = global_vars.into_iter().map(|(_, v)| v).collect();

        Self {
            stack,
            functions: vec![],
            predefined_types: pt,
            const_table,
            heap: SimpleHeap::new(),
            call_frames: vec![CallFrame::default()],
            break_flag: false,
        }
    }

    fn push_frame(&mut self, upvalues: Vec<ObjectId>) {
        self.call_frames.push(CallFrame {
            base: self.stack.len(),
            upvalues,
        });
    }

    fn pop_frames(&mut self) {
        let frame = self.call_frames.pop().unwrap();
        self.stack.truncate(frame.base);
    }

    fn current_frame(&self) -> &CallFrame {
        self.call_frames.last().unwrap()
    }

    fn set_stack_value(&mut self, index: usize, value: Value) {
        if self.stack.len() <= index {
            self.stack.resize(index + 1, Value::Void);
        }
        self.stack[index] = value;
    }

    pub fn declare(&mut self, slot: Slot, value: Value) {
        let index = self.current_frame().base + slot.0;
        self.set_stack_value(index, value);
    }

    fn deref_value(&self, mut value: Value) -> Value {
        while let Value::Ref(obj_id) = value {
            value = self.heap.fetch_obj(obj_id).clone();
        }
        value
    }

    pub fn eval(&mut self, expr: &ir::Ir) -> Result<Value, Failure> {
        let value = match &expr.kind {
            ExprKind::Nop => Value::Void,
            ExprKind::Int(value) => Value::Integer(*value),
            ExprKind::Float(value) => Value::Float(*value),
            ExprKind::Char(value) => Value::Char(*value),
            ExprKind::Char32(value) => Value::Char32(*value),
            ExprKind::String(const_id) => {
                let ConstValue::String(str) = &self.const_table[const_id.0];
                Value::String(str.clone())
            }
            ExprKind::Logic(value) => Value::Logic(*value),
            ExprKind::Type(e) => Value::Type(*e),
            ExprKind::LoadGlobal { slot } => self.handle_load_global(*slot)?,
            ExprKind::StoreGlobal { slot, value } => self.handle_store_global(*slot, value)?,
            ExprKind::LoadLocal { slot } => self.handle_load_local(*slot)?,
            ExprKind::StoreLocal { slot, value } => self.handle_store_local(*slot, value)?,
            ExprKind::LoadUpvalue { index } => self.handle_load_upvalue(*index)?,
            ExprKind::StoreUpvalue { index, value } => self.handle_store_upvalue(*index, value)?,
            ExprKind::Neg(expr) => -(self.eval(expr)?),
            ExprKind::Not(ir) => self.handle_not(ir)?,
            ExprKind::If(expr) => self.eval_if(expr)?,
            ExprKind::Loop(ir) => self.eval_loop(ir)?,
            ExprKind::Break => self.eval_break(),
            ExprKind::Template(expr) => self.eval_template(expr)?,
            ExprKind::CompareChain(expr) => self.eval_compare_chain(expr)?,
            ExprKind::Tuple(e) => self.eval_tuple(e, expr.ty)?,
            ExprKind::Block(expr) => self.eval_block(expr)?,
            ExprKind::Func(e) => self.eval_func_expr(e)?,
            ExprKind::Call(expr) => self.eval_call(expr)?,
            ExprKind::Cast { ty, value } => self.test_value_type(value, *ty)?,
            ExprKind::IndexTuple { tuple, index } => {
                if let Value::Tuple { elements, .. } = self.eval(&tuple)? {
                    elements[*index].clone()
                } else {
                    panic!("GetTupleElem on a non-tuple value")
                }
            }
            ExprKind::GetLength(id) => self.eval_get_length(id)?,
            ExprKind::Option(id) => self.eval_option_value(id.as_deref())?,
            _ => unimplemented!(),
        };
        Ok(self.deref_value(value))
    }

    fn handle_load_global(&self, slot: Slot) -> Result<Value, Failure> {
        let value = &self.stack[slot.0];
        Ok(value.clone())
    }

    fn handle_store_global(&mut self, slot: Slot, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        self.set_stack_value(slot.0, value.clone());
        Ok(value)
    }

    fn handle_load_local(&mut self, slot: Slot) -> Result<Value, Failure> {
        let index = self.current_frame().base + slot.0;
        Ok(self.stack[index].clone())
    }

    fn handle_store_local(&mut self, slot: Slot, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        let index = self.current_frame().base + slot.0;
        if index < self.stack.len() {
            if let Value::Ref(obj_id) = self.stack[index] {
                self.heap.update_obj(obj_id, value.clone());
                return Ok(value);
            }
        }
        self.set_stack_value(index, value.clone());
        Ok(value)
    }

    fn handle_load_upvalue(&self, index: usize) -> Result<Value, Failure> {
        let obj_id = self.current_frame().upvalues[index];
        let value = self.heap.fetch_obj(obj_id).clone();
        Ok(value)
    }

    fn handle_store_upvalue(&mut self, index: usize, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        let obj_id = self.current_frame().upvalues[index];
        self.heap.update_obj(obj_id, value.clone());
        Ok(value)
    }

    fn eval_option_value(&mut self, expr_id: Option<&Ir>) -> Result<Value, Failure> {
        if let Some(id) = expr_id {
            let value = self.eval(id)?;
            Ok(Value::Option(Some(value.into())))
        } else {
            Ok(Value::Option(None))
        }
    }

    fn eval_call(&mut self, expr: &ir::CallExpr) -> Result<Value, Failure> {
        match self.eval(expr.callee.as_ref())? {
            Value::Function { kind } => match kind {
                FnKind::Native(func) => {
                    let args: Result<Vec<_>, _> =
                        expr.args.iter().map(|arg| self.eval(arg)).collect();
                    args.and_then(|args| {
                        let mut ctx = CallContext {
                            heap: &self.heap,
                            args: &args,
                            ret_val: None,
                        };
                        func(&mut ctx);
                        ctx.ret_val
                            .unwrap_or(Ok(Value::Void))
                            .map_err(|_| Failure())
                    })
                }
                FnKind::Verse { id, upvalues } => {
                    let func_expr = &self.functions[id.0].clone();
                    let args: Result<Vec<_>, _> =
                        expr.args.iter().map(|arg| self.eval(arg)).collect();
                    let args = args?;
                    self.push_frame(upvalues);
                    let value = (|| -> Result<Value, Failure> {
                        for (param, arg) in func_expr.params.iter().zip(args.into_iter()) {
                            self.declare(*param, arg);
                        }
                        let ret_val = self.eval(func_expr.body.as_ref())?;
                        Ok(if func_expr.return_void {
                            Value::Void
                        } else {
                            ret_val
                        })
                    })();
                    self.pop_frames();
                    value
                }
            },
            _ => panic!("callee is not callable"),
        }
    }

    fn test_value_type(&mut self, value: &Ir, type_id: TypeId) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        let ok = match &value {
            Value::Integer(_) => type_id == self.predefined_types.t_int,
            Value::Tuple { ty, .. } => *ty == type_id,
            _ => false,
        };
        if ok { Ok(value) } else { Err(Failure()) }
    }

    fn handle_not(&mut self, ir: &ir::Ir) -> Result<Value, Failure> {
        let value = self.eval(ir);
        match value {
            Ok(Value::Logic(v)) => Ok(Value::Logic(!v)),
            _ => Ok(Value::Logic(!value.is_ok())),
        }
    }

    fn eval_if(&mut self, expr: &ir::IfExpr) -> Result<Value, Failure> {
        if let Ok(test) = self.eval(&expr.test) {
            if !matches!(test, Value::Logic(false)) {
                self.eval(&expr.then)
            } else if let Some(alternate) = &expr.alt {
                self.eval(alternate)
            } else {
                Ok(Value::Void)
            }
        } else {
            if let Some(alternate) = &expr.alt {
                let value = self.eval(alternate)?;
                Ok(Value::Option(Some(value.into())))
            } else {
                Ok(Value::Option(None))
            }
        }
    }

    fn eval_loop(&mut self, ir: &Ir) -> Result<Value, Failure> {
        self.break_flag = false;
        loop {
            self.eval(ir)?;
            if self.break_flag {
                self.break_flag = false;
                break;
            }
        }
        Ok(Value::Logic(true))
    }

    fn eval_break(&mut self) -> Value {
        self.break_flag = true;
        Value::Void
    }

    fn eval_template(&mut self, elements: &[ir::TemplateElement]) -> Result<Value, Failure> {
        let elems: Result<Vec<_>, _> = elements
            .iter()
            .map(|el| match el {
                ir::TemplateElement::String(const_id) => {
                    let ConstValue::String(s) = &self.const_table[const_id.0];
                    Ok(s.clone())
                }
                ir::TemplateElement::Expr(expr) => self.eval(expr).map(|v| {
                    let mut s = String::new();
                    write_value(&mut s, &self.heap, &v, false).unwrap();
                    s
                }),
            })
            .collect();

        Ok(Value::String(elems?.concat()))
    }

    fn eval_compare_chain(&mut self, expr: &ir::CompareChainExpr) -> Result<Value, Failure> {
        let leftmost = self.eval(&expr.head)?;
        let mut prev = leftmost.clone();

        for (op, expr) in &expr.rest {
            let current = self.eval(expr)?;

            match op {
                CompareOp::Eq => {
                    if !(prev == current) {
                        return Err(Failure());
                    }
                }
                CompareOp::Ne => {
                    if !(prev != current) {
                        return Err(Failure());
                    }
                }
                CompareOp::Gt => {
                    if !(prev > current) {
                        return Err(Failure());
                    }
                }
                CompareOp::Ge => {
                    if !(prev >= current) {
                        return Err(Failure());
                    }
                }
                CompareOp::Lt => {
                    if !(prev < current) {
                        return Err(Failure());
                    }
                }
                CompareOp::Le => {
                    if !(prev <= current) {
                        return Err(Failure());
                    }
                }
            }

            prev = current;
        }

        Ok(leftmost)
    }

    fn eval_tuple(&mut self, elements: &[Ir], ty: TypeId) -> Result<Value, Failure> {
        let elems: Result<Vec<_>, _> = elements.iter().map(|el| self.eval(el)).collect();
        Ok(Value::Tuple {
            ty,
            elements: elems?,
        })
    }

    fn eval_block(&mut self, expr_ids: &[Ir]) -> Result<Value, Failure> {
        let mut result = Ok(Value::Void);
        for expr in expr_ids {
            result = self.eval(expr);
            if result.is_err() {
                break;
            }
        }
        result
    }

    fn eval_func_expr(&mut self, expr: &ir::FunctionExpr) -> Result<Value, Failure> {
        let id = FunctionId(self.functions.len());
        self.functions.push(expr.clone());

        let base = self.current_frame().base;
        let upvalues: Vec<ObjectId> = expr
            .upvalues
            .iter()
            .map(|desc| match desc {
                UpvalueDesc::Local(slot) => {
                    let index = base + slot.0;
                    match &self.stack[index] {
                        Value::Ref(obj_id) => *obj_id,
                        _ => {
                            let value = std::mem::take(&mut self.stack[index]);
                            let obj_id = self.heap.alloc_obj(value);
                            self.stack[index] = Value::Ref(obj_id);
                            obj_id
                        }
                    }
                }
                UpvalueDesc::Upvalue(index) => self.current_frame().upvalues[*index],
            })
            .collect();

        let func = Value::Function {
            kind: FnKind::Verse { id, upvalues },
        };

        self.declare(expr.slot, func.clone());

        Ok(func)
    }

    fn eval_get_length(&mut self, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        match value {
            Value::String(str) => Ok(Value::Integer(str.len() as i64)),
            _ => panic!("unsupported GetLength target"),
        }
    }
}
