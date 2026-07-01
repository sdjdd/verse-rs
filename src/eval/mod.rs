use std::collections::{HashMap, HashSet};

use crate::{
    ast::*,
    core::{Symbol, SymbolTable},
    runtime::{CallContext, Failure, FunctionId, FunctionKind, Value, builtin_funcs},
};

#[derive(Default)]
struct EvalScope {
    bindings: HashMap<Symbol, Value>,
}

pub struct Evaluator {
    symbol_table: SymbolTable,
    scopes: Vec<EvalScope>,
    functions: HashMap<FunctionId, FunctionExpr>,
    void_funcs: HashSet<FunctionId>,
}

impl Evaluator {
    pub fn new(mut symbol_table: SymbolTable, void_funcs: &[FunctionId]) -> Self {
        let mut bindings = HashMap::new();

        bindings.insert(
            symbol_table.intern("Print"),
            Value::Function {
                kind: FunctionKind::Native(builtin_funcs::print),
            },
        );

        let root_scope = EvalScope {
            bindings,
            ..Default::default()
        };

        Self {
            scopes: vec![root_scope],
            symbol_table,
            functions: HashMap::new(),
            void_funcs: void_funcs.iter().cloned().collect(),
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

    pub fn eval(&mut self, expr: &Expression) -> Result<Value, Failure> {
        match &expr.kind {
            ExprKind::Call(expr) => self.eval_call(expr),
            ExprKind::Integer(value) => Ok(Value::Integer(*value)),
            ExprKind::Float(value) => Ok(Value::Float(*value)),
            ExprKind::Char(value) => Ok(Value::Char(*value)),
            ExprKind::Char32(value) => Ok(Value::Char32(*value)),
            ExprKind::String(value) => Ok(Value::String(value.clone())),
            ExprKind::Logic(value) => Ok(Value::Logic(*value)),
            ExprKind::Decl(expr) => self.eval_declaration(expr),
            ExprKind::VarDecl(expr) => self.eval_var_decl(expr),
            ExprKind::Set(expr) => self.eval_set(expr),
            ExprKind::Id(expr) => self.eval_identifier(expr),
            ExprKind::Binary(expr) => self.eval_binary(expr),
            ExprKind::If(expr) => self.eval_if(expr),
            ExprKind::Template(expr) => self.eval_template(expr),
            ExprKind::CompareChain(expr) => self.eval_compare_chain(expr),
            ExprKind::Tuple(expr) => self.eval_tuple(expr),
            ExprKind::Block(expr) => self.eval_block(expr),
            ExprKind::Func(e) => self.eval_func_expr(e, expr.id),
        }
    }

    fn eval_declaration(&mut self, expr: &DeclExpr) -> Result<Value, Failure> {
        let value = self.eval(&expr.value)?;
        self.declare(expr.target, value.clone());
        Ok(value)
    }

    fn eval_set(&mut self, expr: &SetExpr) -> Result<Value, Failure> {
        let value = self.eval(&expr.expr)?;
        match &expr.target.kind {
            LValueKind::Id(id) => {
                self.declare(id.symbol, value.clone());
            }
        }
        Ok(value)
    }

    fn eval_var_decl(&mut self, expr: &VarDeclExpr) -> Result<Value, Failure> {
        let value = self.eval(&expr.expr)?;
        self.declare(expr.name.symbol, value.clone());
        Ok(value)
    }

    fn eval_identifier(&mut self, expr: &IdExpr) -> Result<Value, Failure> {
        if let Some(value) = self.resolve_symbol(expr.symbol) {
            Ok(value.clone())
        } else {
            panic!("{} is not defined", self.symbol_table.resolve(expr.symbol))
        }
    }

    fn eval_call(&mut self, expr: &CallExpr) -> Result<Value, Failure> {
        match self.eval(&expr.callee)? {
            Value::Tuple(elements) => self.eval_call_tuple(&elements, &expr.args),
            Value::Function { kind } => match kind {
                FunctionKind::Native(func) => {
                    let args: Result<Vec<_>, _> =
                        expr.args.iter().map(|arg| self.eval(arg)).collect();
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
                    self.push_scope();

                    let val = {
                        let func_expr = self.functions[&func_id].clone();
                        let args: Result<Vec<_>, _> =
                            expr.args.iter().map(|arg| self.eval(arg)).collect();

                        args.map(|args| {
                            for (i, param) in func_expr.params.iter().enumerate() {
                                self.declare(param.name, args[i].clone());
                            }
                        })
                        .and_then(|_| {
                            self.eval(&func_expr.body).map(|ret_val| {
                                if self.void_funcs.contains(&func_id) {
                                    Value::Void
                                } else {
                                    ret_val
                                }
                            })
                        })
                    };

                    self.pop_scope();

                    val
                }
            },
            _ => unimplemented!(),
        }
    }

    fn eval_call_tuple(
        &mut self,
        elements: &[Value],
        arguments: &[Expression],
    ) -> Result<Value, Failure> {
        let arg = self.eval(&arguments[0])?;

        match arg {
            Value::Integer(idx) => Ok(elements[idx as usize].clone()),
            _ => unimplemented!(),
        }
    }

    fn eval_binary(&mut self, expr: &BinaryExpr) -> Result<Value, Failure> {
        let left = self.eval(&expr.lhs)?;
        let right = self.eval(&expr.rhs)?;
        match (left, right) {
            (left, right) => {
                let value = match expr.op {
                    BinaryOperator::Plus => match (&left, &right) {
                        (Value::Integer(l), Value::Integer(r)) => Value::Integer(l + r),
                        (Value::Float(l), Value::Float(r)) => Value::Float(l + r),
                        _ if let (Some(a), Some(b)) = (left.to_rational(), right.to_rational()) => {
                            Value::rational(a.0 * b.1 + b.0 * a.1, a.1 * b.1)
                        }
                        _ => unimplemented!(),
                    },
                    BinaryOperator::Sub => match (&left, &right) {
                        (Value::Integer(l), Value::Integer(r)) => Value::Integer(l - r),
                        (Value::Float(l), Value::Float(r)) => Value::Float(l - r),
                        _ if let (Some(a), Some(b)) = (left.to_rational(), right.to_rational()) => {
                            Value::rational(a.0 * b.1 - b.0 * a.1, a.1 * b.1)
                        }
                        _ => unimplemented!(),
                    },
                    BinaryOperator::Mul => match (&left, &right) {
                        (Value::Integer(l), Value::Integer(r)) => Value::Integer(l * r),
                        (Value::Float(l), Value::Float(r)) => Value::Float(l * r),
                        _ if let (Some(a), Some(b)) = (left.to_rational(), right.to_rational()) => {
                            Value::rational(a.0 * b.0, a.1 * b.1)
                        }
                        _ => unimplemented!(),
                    },
                    BinaryOperator::Div => match (&left, &right) {
                        (Value::Integer(_), Value::Integer(0)) => return Err(Failure()),
                        (Value::Integer(l), Value::Integer(r)) => Value::rational(*l, *r),
                        (Value::Float(l), Value::Float(r)) => Value::Float(l / r),
                        _ if let (Some(a), Some(b)) = (left.to_rational(), right.to_rational()) => {
                            if b.0 == 0 {
                                return Err(Failure());
                            }
                            Value::rational(a.0 * b.1, a.1 * b.0)
                        }
                        _ => unimplemented!(),
                    },
                };
                Ok(value)
            }
        }
    }

    fn eval_if(&mut self, expr: &IfExpr) -> Result<Value, Failure> {
        if let Ok(test) = self.eval(&expr.test) {
            if !matches!(test, Value::Logic(false)) {
                self.eval(&expr.consequent)
            } else if let Some(alternate) = &expr.alternate {
                self.eval(alternate)
            } else {
                Ok(Value::Void)
            }
        } else {
            if let Some(alternate) = &expr.alternate {
                self.eval(alternate)
            } else {
                Err(Failure())
            }
        }
    }

    fn eval_template(&mut self, expr: &TemplateExpression) -> Result<Value, Failure> {
        let mut strings = Vec::new();
        strings.reserve(expr.elements.len());
        for elem in expr.elements.iter() {
            match elem {
                TemplateElement::Raw(str) => strings.push(str.clone()),
                TemplateElement::Expr(expr) => {
                    if let Ok(value) = self.eval(expr) {
                        strings.push(value.to_string());
                    } else {
                        return Err(Failure());
                    }
                }
            }
        }
        Ok(Value::String(strings.concat()))
    }

    fn eval_compare_chain(&mut self, expr: &CompareChainExpr) -> Result<Value, Failure> {
        let leftmost = if let Ok(value) = self.eval(&expr.head) {
            value
        } else {
            return Err(Failure());
        };

        let mut prev = leftmost.clone();

        for (op, expr) in &expr.rest {
            let current = if let Ok(value) = self.eval(expr) {
                value
            } else {
                return Err(Failure());
            };

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

    fn eval_tuple(&mut self, expr: &TupleExpr) -> Result<Value, Failure> {
        let mut values = Vec::new();
        values.reserve(expr.elements.len());
        for expr in &expr.elements {
            if let Ok(value) = self.eval(expr) {
                values.push(value);
            } else {
                return Err(Failure());
            }
        }
        Ok(Value::Tuple(values))
    }

    fn eval_block(&mut self, expr: &BlockExpr) -> Result<Value, Failure> {
        let mut result = Ok(Value::Void);
        for expr in &expr.body {
            result = self.eval(expr);
            if result.is_err() {
                break;
            }
        }
        result
    }

    fn eval_func_expr(&mut self, expr: &FunctionExpr, expr_id: ExprId) -> Result<Value, Failure> {
        self.declare(
            expr.name,
            Value::Function {
                kind: FunctionKind::Verse(FunctionId(expr_id.0)),
            },
        );
        self.functions.insert(FunctionId(expr_id.0), expr.clone());
        Ok(Value::Function {
            kind: FunctionKind::Verse(FunctionId(expr_id.0)),
        })
    }
}
