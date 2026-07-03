use std::collections::HashMap;

use crate::{
    ast::{BinaryOperator, CompareOp},
    core::{ConstTable, ConstValue, Symbol},
    ir,
    runtime::{CallContext, Failure, FunctionId, FunctionKind, TypeId, Value, builtin_funcs},
    semantic::builtins::{BuiltinSymbols, BuiltinTypes},
};

#[derive(Default)]
struct EvalScope {
    bindings: HashMap<Symbol, Value>,
}

pub struct Evaluator {
    scopes: Vec<EvalScope>,
    functions: HashMap<FunctionId, ir::FunctionExpr>,
    builtin_types: BuiltinTypes,
    irs: Vec<ir::Ir>,
    const_table: ConstTable,
}

impl Evaluator {
    pub fn new(
        builtin_symbols: BuiltinSymbols,
        builtin_types: BuiltinTypes,
        const_table: ConstTable,
        irs: Vec<ir::Ir>,
    ) -> Self {
        let mut bindings = HashMap::new();

        bindings.insert(
            builtin_symbols.s_Print,
            Value::Function {
                kind: FunctionKind::Native(builtin_funcs::print),
            },
        );

        for (s, t) in builtin_types.pairs(&builtin_symbols) {
            bindings.insert(s, Value::Type(t));
        }

        let root_scope = EvalScope {
            bindings,
            ..Default::default()
        };

        Self {
            scopes: vec![root_scope],
            functions: HashMap::new(),
            builtin_types: builtin_types,
            irs,
            const_table,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(EvalScope::default());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, symbol: Symbol, value: Value) {
        self.scopes
            .last_mut()
            .unwrap()
            .bindings
            .insert(symbol, value);
    }

    pub fn resolve_symbol(&self, symbol: Symbol) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.bindings.get(&symbol) {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn eval(&mut self, expr: ir::ExprId) -> Result<Value, Failure> {
        let hir_expr = &self.irs[expr.0].clone();

        use ir::ExprKind;
        match &hir_expr.kind {
            ExprKind::Int(value) => Ok(Value::Integer(*value)),
            ExprKind::Float(value) => Ok(Value::Float(*value)),
            ExprKind::Char(value) => Ok(Value::Char(*value)),
            ExprKind::Char32(value) => Ok(Value::Char32(*value)),
            ExprKind::String(const_id) => {
                let ConstValue::String(s) = self.const_table.get(*const_id).unwrap();
                Ok(Value::String(s.clone()))
            }
            ExprKind::Logic(value) => Ok(Value::Logic(*value)),
            ExprKind::Type(e) => Ok(Value::Type(*e)),
            ExprKind::Set(expr) => self.eval_set(expr),
            ExprKind::Id(s) => self.eval_identifier(*s),
            ExprKind::Binary(expr) => self.eval_binary(expr),
            ExprKind::If(expr) => self.eval_if(expr),
            ExprKind::Template(expr) => self.eval_template(expr),
            ExprKind::CompareChain(expr) => self.eval_compare_chain(expr),
            ExprKind::Tuple(e) => self.eval_tuple(e, hir_expr.ty),
            ExprKind::Block(expr) => self.eval_block(expr),
            ExprKind::Func(e) => self.eval_func_expr(e, hir_expr.id),
            ExprKind::Call(expr) => self.eval_call(expr),
            ExprKind::Cast { ty, value } => self.test_value_type(*value, *ty),
            ExprKind::NoOp => Ok(Value::Void),
            ExprKind::GetTupleElem { tuple, index } => {
                if let Value::Tuple { elements, .. } = self.eval(*tuple)? {
                    Ok(elements[*index].clone())
                } else {
                    panic!("GetTupleElem on a non-tuple value")
                }
            }
        }
    }

    fn eval_set(&mut self, expr: &ir::SetExpr) -> Result<Value, Failure> {
        let value = self.eval(expr.value)?;
        self.declare(expr.target, value.clone());
        Ok(value)
    }

    fn eval_identifier(&mut self, symbol: Symbol) -> Result<Value, Failure> {
        if let Some(value) = self.resolve_symbol(symbol) {
            Ok(value.clone())
        } else {
            panic!("identifier not defined, symbol: {:?}", symbol)
        }
    }

    fn eval_call(&mut self, expr: &ir::CallExpr) -> Result<Value, Failure> {
        match self.eval(expr.callee)? {
            Value::Function { kind } => match kind {
                FunctionKind::Native(func) => {
                    let args: Result<Vec<_>, _> =
                        expr.args.iter().map(|arg| self.eval(*arg)).collect();
                    args.and_then(|args| {
                        let mut ctx = CallContext {
                            args: &args,
                            ret_val: None,
                        };
                        func(&mut ctx);
                        ctx.ret_val
                            .unwrap_or(Ok(Value::Void))
                            .map_err(|_| Failure())
                    })
                }
                FunctionKind::Verse(func_id) => {
                    let func_expr = &self.functions[&func_id].clone();
                    self.push_scope();
                    let val = (|| -> Result<Value, Failure> {
                        for (param, arg) in func_expr.params.iter().zip(expr.args.iter()) {
                            let arg = self.eval(*arg)?;
                            self.declare(*param, arg);
                        }
                        let ret_val = self.eval(func_expr.body)?;
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

    fn test_value_type(&mut self, value: ir::ExprId, type_id: TypeId) -> Result<Value, Failure> {
        let value = self.eval(value)?;
        let ok = match &value {
            Value::Integer(_) => type_id == self.builtin_types.t_int,
            Value::Tuple { ty, .. } => *ty == type_id,
            _ => false,
        };
        if ok { Ok(value) } else { Err(Failure()) }
    }

    fn eval_binary(&mut self, expr: &ir::BinaryExpr) -> Result<Value, Failure> {
        let left = self.eval(expr.lhs)?;
        let right = self.eval(expr.rhs)?;
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
        if let Ok(test) = self.eval(expr.test) {
            if !matches!(test, Value::Logic(false)) {
                self.eval(expr.then)
            } else if let Some(alternate) = &expr.alt {
                self.eval(*alternate)
            } else {
                Ok(Value::Void)
            }
        } else {
            if let Some(alternate) = &expr.alt {
                // TODO: optional
                self.eval(*alternate)
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
                    let ConstValue::String(s) = self.const_table.get(*const_id).unwrap();
                    Ok(s.clone())
                }
                ir::TemplateElement::Expr(expr) => self.eval(*expr).map(|v| v.to_string()),
            })
            .collect();

        elems.map(|elems| Value::String(elems.concat()))
    }

    fn eval_compare_chain(&mut self, expr: &ir::CompareChainExpr) -> Result<Value, Failure> {
        let leftmost = self.eval(expr.head)?;
        let mut prev = leftmost.clone();

        for (op, expr) in &expr.rest {
            let current = self.eval(*expr)?;

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

    fn eval_tuple(&mut self, elements: &[ir::ExprId], ty: TypeId) -> Result<Value, Failure> {
        let elems: Result<Vec<_>, _> = elements.iter().map(|el| self.eval(*el)).collect();
        elems.map(|elems| Value::Tuple {
            ty,
            elements: elems,
        })
    }

    fn eval_block(&mut self, expr_ids: &[ir::ExprId]) -> Result<Value, Failure> {
        let mut result = Ok(Value::Void);
        for expr in expr_ids {
            result = self.eval(*expr);
            if result.is_err() {
                break;
            }
        }
        result
    }

    fn eval_func_expr(
        &mut self,
        expr: &ir::FunctionExpr,
        expr_id: ir::ExprId,
    ) -> Result<Value, Failure> {
        let func_id = FunctionId(expr_id.0);
        self.declare(
            expr.name,
            Value::Function {
                kind: FunctionKind::Verse(func_id),
            },
        );
        self.functions.insert(func_id, expr.clone());
        Ok(Value::Function {
            kind: FunctionKind::Verse(func_id),
        })
    }
}
