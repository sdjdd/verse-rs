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

pub struct EvalContext {
    symbol_table: SymbolTable,
    scopes: Vec<EvalScope>,
    functions: HashMap<FunctionId, FunctionExpr>,
    void_funcs: HashSet<FunctionId>,
}

impl EvalContext {
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

    fn declare_function(&mut self, id: FunctionId, expr: FunctionExpr) {
        self.functions.insert(id, expr);
    }

    fn lookup_function(&self, id: FunctionId) -> Option<&FunctionExpr> {
        self.functions.get(&id)
    }
}

pub fn eval(expr: &Expression, ctx: &mut EvalContext) -> Result<Value, Failure> {
    match &expr.kind {
        ExprKind::Call(expr) => eval_call(expr, ctx),
        ExprKind::Integer(value) => Ok(Value::Integer(*value)),
        ExprKind::Float(value) => Ok(Value::Float(*value)),
        ExprKind::Char(value) => Ok(Value::Char(*value)),
        ExprKind::Char32(value) => Ok(Value::Char32(*value)),
        ExprKind::String(value) => Ok(Value::String(value.clone())),
        ExprKind::Logic(value) => Ok(Value::Logic(*value)),
        ExprKind::Decl(expr) => eval_declaration(expr, ctx),
        ExprKind::VarDecl(expr) => eval_var_decl(expr, ctx),
        ExprKind::Set(expr) => eval_set(expr, ctx),
        ExprKind::Id(expr) => eval_identifier(expr, ctx),
        ExprKind::Binary(expr) => eval_binary(expr, ctx),
        ExprKind::If(expr) => eval_if(expr, ctx),
        ExprKind::Template(expr) => eval_template(expr, ctx),
        ExprKind::CompareChain(expr) => eval_compare_chain(expr, ctx),
        ExprKind::Tuple(expr) => eval_tuple(expr, ctx),
        ExprKind::Block(expr) => eval_block(expr, ctx),
        ExprKind::Func(e) => eval_func_expr(e, ctx, expr.id),
    }
}

fn eval_declaration(expr: &DeclarationExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let value = eval(&expr.value, ctx)?;
    ctx.declare(expr.target, value.clone());
    Ok(value)
}

fn eval_set(expr: &SetExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let value = eval(&expr.expr, ctx)?;
    match &expr.target.kind {
        LValueKind::Id(id) => {
            ctx.declare(id.symbol, value.clone());
        }
    }
    Ok(value)
}

fn eval_var_decl(expr: &VarDeclExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let value = eval(&expr.expr, ctx)?;
    ctx.declare(expr.name.symbol, value.clone());
    Ok(value)
}

fn eval_identifier(expr: &IdentifierExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    if let Some(value) = ctx.resolve_symbol(expr.symbol) {
        Ok(value.clone())
    } else {
        panic!("{} is not defined", ctx.symbol_table.resolve(expr.symbol))
    }
}

fn eval_call(expr: &CallExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    match &expr.callee.kind {
        ExprKind::Id(id) => {
            let value = ctx.resolve_symbol(id.symbol).unwrap();
            match value {
                Value::Tuple(elements) => Ok(eval_call_tuple(&elements, &expr.args, ctx)?),
                Value::Function { kind } => match kind {
                    FunctionKind::Native(func) => {
                        let args: Result<Vec<_>, _> =
                            expr.args.iter().map(|arg| eval(arg, ctx)).collect();
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
                        ctx.push_scope();

                        let val = {
                            let func_expr = ctx.lookup_function(func_id).unwrap().clone();
                            let args: Result<Vec<_>, _> =
                                expr.args.iter().map(|arg| eval(arg, ctx)).collect();

                            args.map(|args| {
                                for (i, param) in func_expr.params.iter().enumerate() {
                                    ctx.declare(param.name, args[i].clone());
                                }
                            })
                            .and_then(|_| {
                                eval(&func_expr.body, ctx).map(|ret_val| {
                                    if ctx.void_funcs.contains(&func_id) {
                                        Value::Void
                                    } else {
                                        ret_val
                                    }
                                })
                            })
                        };

                        ctx.pop_scope();

                        val
                    }
                },
                _ => unimplemented!(),
            }
        }
        _ => unimplemented!(),
    }
}

fn eval_call_tuple(
    elements: &[Value],
    arguments: &[Expression],
    ctx: &mut EvalContext,
) -> Result<Value, Failure> {
    if arguments.len() != 1 {
        panic!("expected 1 arguments, found {}", arguments.len())
    }

    let arg = eval(&arguments[0], ctx)?;

    match arg {
        Value::Integer(idx) => {
            if idx >= 0 && idx < elements.len() as i64 {
                Ok(elements[idx as usize].clone())
            } else {
                panic!(
                    "index out of bounds: the length is {} but the index is {}",
                    elements.len(),
                    idx
                )
            }
        }
        _ => unimplemented!(),
    }
}

fn eval_binary(expr: &BinaryExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let left = eval(&expr.lhs, ctx)?;
    let right = eval(&expr.rhs, ctx)?;
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

fn eval_if(expr: &IfExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    if let Ok(test) = eval(&expr.test, ctx) {
        if !matches!(test, Value::Logic(false)) {
            eval(&expr.consequent, ctx)
        } else if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Value::Void)
        }
    } else {
        if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Err(Failure())
        }
    }
}

fn eval_template(expr: &TemplateExpression, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let mut strings = Vec::new();
    strings.reserve(expr.elements.len());
    for elem in expr.elements.iter() {
        match elem {
            TemplateElement::Raw(str) => strings.push(str.clone()),
            TemplateElement::Expr(expr) => {
                if let Ok(value) = eval(expr, ctx) {
                    strings.push(value.to_string());
                } else {
                    return Err(Failure());
                }
            }
        }
    }
    Ok(Value::String(strings.concat()))
}

fn eval_compare_chain(expr: &CompareChainExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let leftmost = if let Ok(value) = eval(&expr.head, ctx) {
        value
    } else {
        return Err(Failure());
    };

    let mut prev = leftmost.clone();

    for (op, expr) in &expr.rest {
        let current = if let Ok(value) = eval(expr, ctx) {
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

fn eval_tuple(expr: &TupleExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let mut values = Vec::new();
    values.reserve(expr.elements.len());
    for expr in &expr.elements {
        if let Ok(value) = eval(expr, ctx) {
            values.push(value);
        } else {
            return Err(Failure());
        }
    }
    Ok(Value::Tuple(values))
}

fn eval_block(expr: &BlockExpr, ctx: &mut EvalContext) -> Result<Value, Failure> {
    let mut result = Ok(Value::Void);
    for expr in &expr.body {
        result = eval(expr, ctx);
        if result.is_err() {
            break;
        }
    }
    result
}

fn eval_func_expr(
    expr: &FunctionExpr,
    ctx: &mut EvalContext,
    expr_id: ExprId,
) -> Result<Value, Failure> {
    ctx.declare(
        expr.name,
        Value::Function {
            kind: FunctionKind::Verse(FunctionId(expr_id.0)),
        },
    );
    ctx.declare_function(FunctionId(expr_id.0), expr.clone());
    Ok(Value::Function {
        kind: FunctionKind::Verse(FunctionId(expr_id.0)),
    })
}
