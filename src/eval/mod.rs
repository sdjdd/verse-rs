use crate::{
    ast::{BinaryOperator, CompareOp},
    core::{ConstValue, PredefinedSymbols, types::PredefinedTypes},
    ir::{self, ExprKind, FunctionExpr, Ir, Slot},
    runtime::{
        CallContext, Failure, FnKind, FunctionId, TypeId, Value,
        builtin_funcs::{self, write_value},
        heap::{Heap, HeapObj, ObjectId, SimpleHeap},
    },
    semantic::Scope,
};

#[derive(Default)]
struct EvalScope {
    bindings: Vec<Value>,
}

pub struct Evaluator {
    scopes: Vec<EvalScope>,
    functions: Vec<FunctionExpr>,
    builtin_types: PredefinedTypes,
    const_table: Vec<ConstValue>,
    heap: Box<dyn Heap>,
}

impl Evaluator {
    pub fn new(
        ps: PredefinedSymbols,
        pt: PredefinedTypes,
        const_table: Vec<ConstValue>,
        root_scope: &Scope,
    ) -> Self {
        let root_values = [
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

        let mut root_values: Vec<_> = root_values
            .into_iter()
            .map(|(symbol, value)| (root_scope.lookup(symbol).unwrap().slot.0, value))
            .collect();

        root_values.sort_by(|a, b| a.0.cmp(&b.0));
        let bindings = root_values.into_iter().map(|(_, v)| v).collect();

        let root_scope = EvalScope {
            bindings,
            ..Default::default()
        };

        Self {
            scopes: vec![root_scope, EvalScope::default()],
            functions: vec![],
            builtin_types: pt,
            const_table,
            heap: Box::new(SimpleHeap::new()),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(EvalScope::default());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, slot: Slot, value: Value) {
        let bindings = &mut self.scopes.last_mut().unwrap().bindings;
        if bindings.len() < slot.0 + 1 {
            bindings.resize(slot.0 + 1, Value::Void);
        }
        bindings[slot.0] = value
    }

    fn fetch_string(&self, id: ObjectId) -> &str {
        match self.heap.fetch_obj(id) {
            HeapObj::String(s) => s.as_ref(),
            _ => panic!("not a string"),
        }
    }

    pub fn eval(&mut self, expr: &ir::Ir) -> Result<Value, Failure> {
        match &expr.kind {
            ExprKind::Nop => Ok(Value::Void),
            ExprKind::Int(value) => Ok(Value::Integer(*value)),
            ExprKind::Float(value) => Ok(Value::Float(*value)),
            ExprKind::Char(value) => Ok(Value::Char(*value)),
            ExprKind::Char32(value) => Ok(Value::Char32(*value)),
            ExprKind::String(const_id) => {
                let ConstValue::String(str) = &self.const_table[const_id.0];
                let id = self.heap.alloc_obj(HeapObj::String(str.clone()));
                Ok(Value::String(id))
            }
            ExprKind::Logic(value) => Ok(Value::Logic(*value)),
            ExprKind::Type(e) => Ok(Value::Type(*e)),
            ExprKind::StoreLocal { slot, value } => self.eval_set(*slot, value),
            ExprKind::LoadUpvalue { depth, slot } => self.eval_get_local(*depth, *slot),
            ExprKind::Binary(expr) => self.eval_binary(expr),
            ExprKind::If(expr) => self.eval_if(expr),
            ExprKind::Template(expr) => self.eval_template(expr),
            ExprKind::CompareChain(expr) => self.eval_compare_chain(expr),
            ExprKind::Tuple(e) => self.eval_tuple(e, expr.ty),
            ExprKind::Block(expr) => self.eval_block(expr),
            ExprKind::Func(e) => self.eval_func_expr(e),
            ExprKind::Call(expr) => self.eval_call(expr),
            ExprKind::Cast { ty, value } => self.test_value_type(value, *ty),
            ExprKind::GetTupleElem { tuple, index } => {
                if let Value::Tuple { oid, .. } = self.eval(&tuple)? {
                    let elements = match self.heap.fetch_obj(oid) {
                        HeapObj::Vec(elems) => elems,
                        _ => panic!("tuple accidently refs a non-vec object"),
                    };
                    Ok(elements[*index])
                } else {
                    panic!("GetTupleElem on a non-tuple value")
                }
            }
            ExprKind::GetLength(id) => self.eval_get_length(id),
            ExprKind::Option(id) => self.eval_option_value(id.as_deref()),
        }
    }

    fn eval_set(&mut self, slot: Slot, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        self.declare(slot, value);
        Ok(value)
    }

    fn eval_get_local(&self, up: usize, slot: Slot) -> Result<Value, Failure> {
        let value = self.scopes.iter().rev().skip(up).next().unwrap().bindings[slot.0];
        Ok(value)
    }

    fn eval_option_value(&mut self, expr_id: Option<&Ir>) -> Result<Value, Failure> {
        if let Some(id) = expr_id {
            let value = self.eval(id)?;
            let obj_id = self.heap.alloc_obj(HeapObj::Value(value));
            Ok(Value::Option(Some(obj_id)))
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
                            heap: self.heap.as_ref(),
                            args: &args,
                            ret_val: None,
                        };
                        func(&mut ctx);
                        ctx.ret_val
                            .unwrap_or(Ok(Value::Void))
                            .map_err(|_| Failure())
                    })
                }
                FnKind::Verse(func_id) => {
                    let func_expr = &self.functions[func_id.0].clone();
                    self.push_scope();
                    let val = (|| -> Result<Value, Failure> {
                        for (param, arg) in func_expr.params.iter().zip(expr.args.iter()) {
                            let arg = self.eval(arg)?;
                            self.declare(*param, arg);
                        }
                        let ret_val = self.eval(func_expr.body.as_ref())?;
                        Ok(if func_expr.return_void {
                            Value::Void
                        } else {
                            ret_val
                        })
                    })();
                    self.pop_scope();
                    val
                }
            },
            _ => panic!("callee is not callable"),
        }
    }

    fn test_value_type(&mut self, value: &Ir, type_id: TypeId) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        let ok = match &value {
            Value::Integer(_) => type_id == self.builtin_types.t_int,
            Value::Tuple { ty, .. } => *ty == type_id,
            _ => false,
        };
        if ok { Ok(value) } else { Err(Failure()) }
    }

    fn eval_binary(&mut self, expr: &ir::BinaryExpr) -> Result<Value, Failure> {
        let left = self.eval(&expr.lhs)?;
        let right = self.eval(&expr.rhs)?;
        match (left, right) {
            (left, right) => {
                let value = match expr.op {
                    BinaryOperator::Plus => left + right,
                    BinaryOperator::Sub => left - right,
                    BinaryOperator::Mul => left * right,
                    BinaryOperator::Div => {
                        if right.is_zero() {
                            return Err(Failure());
                        }
                        left / right
                    }
                };
                Ok(value)
            }
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
                // TODO: optional
                self.eval(alternate)
            } else {
                Err(Failure())
            }
        }
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
                    write_value(&mut s, self.heap.as_ref(), &v, false).unwrap();
                    s
                }),
            })
            .collect();

        let elems = elems?;
        let id = self.heap.alloc_obj(HeapObj::String(elems.concat()));
        Ok(Value::String(id))
    }

    fn eval_compare_chain(&mut self, expr: &ir::CompareChainExpr) -> Result<Value, Failure> {
        let leftmost = self.eval(&expr.head)?;
        let mut prev = leftmost;

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
        let elems = elems?;
        let oid = self.heap.alloc_obj(HeapObj::Vec(elems));
        Ok(Value::Tuple { ty, oid })
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
        self.declare(
            expr.slot,
            Value::Function {
                kind: FnKind::Verse(id),
            },
        );
        Ok(Value::Function {
            kind: FnKind::Verse(id),
        })
    }

    fn eval_get_length(&mut self, value: &Ir) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        match value {
            Value::String(oid) => {
                let len = self.fetch_string(oid).len();
                Ok(Value::Integer(len as i64))
            }
            _ => panic!("cannot get lenght of non string value"),
        }
    }
}
