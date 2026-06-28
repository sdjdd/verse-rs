use std::collections::HashMap;

use thiserror::Error;

use crate::{
    ast::*,
    core::{Symbol, SymbolTable},
    runtime::{CallContext, Failure, FunctionKind, Value, builtin_funcs},
};

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("ReferenceError: {0}")]
    ReferenceError(String),

    #[error("SyntaxError: {0}")]
    SyntaxError(String),
}

pub type EvalResult<T = Value> = Result<Result<T, Failure>, EvalError>;

pub struct EvalContext {
    bindings: HashMap<Symbol, Value>,
    symbol_table: SymbolTable,
}

impl EvalContext {
    pub fn new(mut symbol_table: SymbolTable) -> Self {
        let mut bindings = HashMap::new();

        bindings.insert(
            symbol_table.intern("Print"),
            Value::Function {
                kind: FunctionKind::Native(builtin_funcs::print),
            },
        );

        Self {
            bindings,
            symbol_table,
        }
    }
}

pub fn eval(expr: &Expression, ctx: &mut EvalContext) -> EvalResult {
    match &expr.kind {
        ExprKind::Call(expr) => eval_call(expr, ctx),
        ExprKind::Integer(value) => Ok(Ok(Value::Integer(*value))),
        ExprKind::Float(value) => Ok(Ok(Value::Float(*value))),
        ExprKind::Char(value) => Ok(Ok(Value::Char(*value))),
        ExprKind::Char32(value) => Ok(Ok(Value::Char32(*value))),
        ExprKind::String(value) => Ok(Ok(Value::String(value.clone()))),
        ExprKind::Logic(value) => Ok(Ok(Value::Logic(*value))),
        ExprKind::Decl(expr) => eval_assignment(expr, ctx),
        ExprKind::VarDecl(expr) => eval_var_decl(expr, ctx),
        ExprKind::Set(expr) => eval_set(expr, ctx),
        ExprKind::Id(expr) => eval_identifier(expr, ctx),
        ExprKind::Binary(expr) => eval_binary(expr, ctx),
        ExprKind::If(expr) => eval_if(expr, ctx),
        ExprKind::Template(expr) => eval_template(expr, ctx),
        ExprKind::CompareChain(expr) => eval_compare_chain(expr, ctx),
        ExprKind::Tuple(expr) => eval_tuple(expr, ctx),
        ExprKind::Block(expr) => eval_block(expr, ctx),
    }
}

fn eval_assignment(expr: &DeclarationExpr, ctx: &mut EvalContext) -> EvalResult {
    eval_set(&SetExpr::new(expr.target.clone(), *expr.value.clone()), ctx)
}

fn eval_set(expr: &SetExpr, ctx: &mut EvalContext) -> EvalResult {
    let value = eval(&expr.expr, ctx)?;
    if let Ok(value) = &value {
        match &expr.target.kind {
            LValueKind::Id(id) => {
                ctx.bindings.insert(id.symbol, value.clone());
            }
        }
    }
    Ok(value)
}

fn eval_var_decl(expr: &VarDeclExpr, ctx: &mut EvalContext) -> EvalResult {
    let value = eval(&expr.expr, ctx)?;
    if let Ok(value) = &value {
        ctx.bindings.insert(expr.name.symbol, value.clone());
    }
    Ok(value)
}

fn eval_identifier(expr: &IdentifierExpr, ctx: &mut EvalContext) -> EvalResult {
    if let Some(value) = ctx.bindings.get(&expr.symbol) {
        Ok(Ok(value.clone()))
    } else {
        Err(EvalError::ReferenceError(format!(
            "{} is not defined",
            ctx.symbol_table.resolve(expr.symbol)
        )))
    }
}

fn eval_call(expr: &CallExpr, ctx: &mut EvalContext) -> EvalResult {
    match &expr.callee.kind {
        ExprKind::Id(id) => {
            let value = ctx.bindings.get(&id.symbol).cloned().unwrap();
            match value {
                Value::Tuple(elements) => Ok(eval_call_tuple(&elements, &expr.args, ctx)?),
                Value::Function { kind } => match kind {
                    FunctionKind::Native(func) => {
                        let args: Result<Result<Vec<_>, _>, _> =
                            expr.args.iter().map(|arg| eval(arg, ctx)).collect();
                        args.and_then(|args| {
                            Ok(args.and_then(|args| {
                                let mut ctx = CallContext {
                                    args: &args,
                                    ret_val: None,
                                };
                                func(&mut ctx);
                                ctx.ret_val
                                    .unwrap_or(Ok(Value::Void))
                                    .map_err(|_| Failure())
                            }))
                        })
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
) -> EvalResult {
    if arguments.len() != 1 {
        return Err(EvalError::SyntaxError(format!(
            "expected 1 arguments, found {}",
            arguments.len()
        )));
    }

    map_eval(ctx, &arguments[0], |arg| match arg {
        Value::Integer(idx) => {
            if idx >= 0 && idx < elements.len() as i64 {
                Ok(elements[idx as usize].clone())
            } else {
                Err(EvalError::SyntaxError(format!(
                    "index out of bounds: the length is {} but the index is {}",
                    elements.len(),
                    idx
                )))
            }
        }
        _ => unimplemented!(),
    })
}

fn eval_binary(expr: &BinaryExpr, ctx: &mut EvalContext) -> EvalResult {
    let left = eval(&expr.lhs, ctx)?;
    let right = eval(&expr.rhs, ctx)?;
    match (left, right) {
        (Ok(left), Ok(right)) => {
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
                    (Value::Integer(_), Value::Integer(0)) => return Ok(Err(Failure())),
                    (Value::Integer(l), Value::Integer(r)) => Value::rational(*l, *r),
                    (Value::Float(l), Value::Float(r)) => Value::Float(l / r),
                    _ if let (Some(a), Some(b)) = (left.to_rational(), right.to_rational()) => {
                        if b.0 == 0 {
                            return Ok(Err(Failure()));
                        }
                        Value::rational(a.0 * b.1, a.1 * b.0)
                    }
                    _ => unimplemented!(),
                },
            };
            Ok(Ok(value))
        }
        _ => Ok(Err(Failure())),
    }
}

fn eval_if(expr: &IfExpr, ctx: &mut EvalContext) -> EvalResult {
    if let Ok(test) = eval(&expr.test, ctx)? {
        if !matches!(test, Value::Logic(false)) {
            eval(&expr.consequent, ctx)
        } else if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Ok(Value::Void))
        }
    } else {
        if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Err(Failure()))
        }
    }
}

fn eval_template(expr: &TemplateExpression, ctx: &mut EvalContext) -> EvalResult {
    let mut strings = Vec::new();
    strings.reserve(expr.elements.len());
    for elem in expr.elements.iter() {
        match elem {
            TemplateElement::Raw(str) => strings.push(str.clone()),
            TemplateElement::Expr(expr) => {
                if let Ok(value) = eval(expr, ctx)? {
                    strings.push(value.to_string());
                } else {
                    return Ok(Err(Failure()));
                }
            }
        }
    }
    Ok(Ok(Value::String(strings.concat())))
}

fn eval_compare_chain(expr: &CompareChainExpr, ctx: &mut EvalContext) -> EvalResult {
    let leftmost = if let Ok(value) = eval(&expr.head, ctx)? {
        value
    } else {
        return Ok(Err(Failure()));
    };

    let mut prev = leftmost.clone();

    for (op, expr) in &expr.rest {
        let current = if let Ok(value) = eval(expr, ctx)? {
            value
        } else {
            return Ok(Err(Failure()));
        };

        match op {
            CompareOp::Eq => {
                if !(prev == current) {
                    return Ok(Err(Failure()));
                }
            }
            CompareOp::Ne => {
                if !(prev != current) {
                    return Ok(Err(Failure()));
                }
            }
            CompareOp::Gt => {
                if !(prev > current) {
                    return Ok(Err(Failure()));
                }
            }
            CompareOp::Ge => {
                if !(prev >= current) {
                    return Ok(Err(Failure()));
                }
            }
            CompareOp::Lt => {
                if !(prev < current) {
                    return Ok(Err(Failure()));
                }
            }
            CompareOp::Le => {
                if !(prev <= current) {
                    return Ok(Err(Failure()));
                }
            }
        }

        prev = current;
    }

    Ok(Ok(leftmost))
}

fn eval_tuple(expr: &TupleExpr, ctx: &mut EvalContext) -> EvalResult {
    let mut values = Vec::new();
    values.reserve(expr.elements.len());
    for expr in &expr.elements {
        if let Ok(value) = eval(expr, ctx)? {
            values.push(value);
        } else {
            return Ok(Err(Failure()));
        }
    }
    Ok(Ok(Value::Tuple(values)))
}

fn eval_block(expr: &BlockExpr, ctx: &mut EvalContext) -> EvalResult {
    let mut result = Ok(Value::Void);
    for expr in &expr.body {
        result = eval(expr, ctx)?;
        if result.is_err() {
            break;
        }
    }
    Ok(result)
}

fn map_eval<F>(ctx: &mut EvalContext, expr: &Expression, op: F) -> EvalResult
where
    F: Fn(Value) -> Result<Value, EvalError>,
{
    match eval(expr, ctx) {
        Ok(Ok(v)) => op(v).map(|v| Ok(v)),
        t => t,
    }
}
