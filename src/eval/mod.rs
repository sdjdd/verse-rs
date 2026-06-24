use std::collections::HashMap;

use thiserror::Error;

use crate::{ast::*, runtime::Value};

#[derive(Debug)]
pub struct Failure();

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("ReferenceError: {0}")]
    ReferenceError(String),

    #[error("SyntaxError: {0}")]
    SyntaxError(String),
}

pub type EvalResult<T = Value> = Result<Result<T, Failure>, EvalError>;

pub struct EvalContext {
    bindings: HashMap<String, Value>,
}

impl EvalContext {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }
}

pub fn eval(expr: &Expression, ctx: &mut EvalContext) -> EvalResult {
    match expr {
        Expression::Call(expr) => eval_call(expr, ctx),
        Expression::Literal(expr) => eval_literal(expr, ctx),
        Expression::Assign(expr) => eval_assignment(expr, ctx),
        Expression::Id(expr) => eval_identifier(expr, ctx),
        Expression::Binary(expr) => eval_binary(expr, ctx),
        Expression::If(expr) => eval_if(expr, ctx),
        Expression::Template(expr) => eval_template(expr, ctx),
        Expression::CompareChain(expr) => eval_compare_chain(expr, ctx),
        Expression::Tuple(expr) => eval_tuple(expr, ctx),
    }
}

fn eval_assignment(expr: &AssignmentExpr, ctx: &mut EvalContext) -> EvalResult {
    let value = eval(&expr.expr, ctx)?;
    if let Ok(value) = &value {
        ctx.bindings.insert(expr.target.clone(), value.clone());
    }
    Ok(value)
}

fn eval_identifier(expr: &IdentifierExpr, ctx: &mut EvalContext) -> EvalResult {
    if let Some(value) = ctx.bindings.get(&expr.name) {
        Ok(Ok(value.clone()))
    } else {
        Err(EvalError::ReferenceError(format!(
            "{} is not defined",
            expr.name
        )))
    }
}

fn eval_literal(expr: &LiteralExpr, _ctx: &mut EvalContext) -> EvalResult {
    let value = match expr {
        LiteralExpr::Integer(value) => Value::Integer(*value),
        LiteralExpr::Float(value) => Value::Float(*value),
        LiteralExpr::Char(value) => Value::Char(*value),
        LiteralExpr::Char32(value) => Value::Char32(*value),
        LiteralExpr::String(value) => Value::String(value.clone()),
        LiteralExpr::Bool(value) => Value::Bool(*value),
    };
    Ok(Ok(value))
}

fn eval_call(expr: &CallExpr, ctx: &mut EvalContext) -> EvalResult {
    match expr.callee.as_ref() {
        Expression::Id(id) => match id.name.as_str() {
            "Print" => {
                if let Some(arg) = expr.arguments.first() {
                    if let Ok(value) = eval(arg, ctx)? {
                        println!("{}", value);
                    }
                }
                Ok(Ok(Value::None))
            }
            name => {
                if let Some(value) = ctx.bindings.get(name).cloned() {
                    match value {
                        Value::Tuple(elements) => {
                            Ok(eval_call_tuple(&elements, &expr.arguments, ctx)?)
                        }
                        _ => unimplemented!(),
                    }
                } else {
                    return Err(EvalError::ReferenceError(format!("{} not found", name)));
                }
            }
        },
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
            if idx >= 0 || idx < elements.len() as i64 {
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
    let left = eval(&expr.left, ctx)?;
    let right = eval(&expr.right, ctx)?;
    match (left, right) {
        (Ok(left), Ok(right)) => {
            let value = match expr.operator {
                BinaryOperator::Plus => match (left, right) {
                    (Value::Integer(l), Value::Integer(r)) => Value::Integer(l + r),
                    (Value::Float(l), Value::Float(r)) => Value::Float(l + r),
                    _ => unimplemented!(),
                },
                BinaryOperator::Sub => match (left, right) {
                    (Value::Integer(l), Value::Integer(r)) => Value::Integer(l - r),
                    (Value::Float(l), Value::Float(r)) => Value::Float(l - r),
                    _ => unimplemented!(),
                },
                BinaryOperator::Mul => match (left, right) {
                    (Value::Integer(l), Value::Integer(r)) => Value::Integer(l * r),
                    (Value::Float(l), Value::Float(r)) => Value::Float(l * r),
                    _ => unimplemented!(),
                },
                BinaryOperator::Div => match (left, right) {
                    (Value::Integer(l), Value::Integer(r)) => Value::Integer(l / r),
                    (Value::Float(l), Value::Float(r)) => Value::Float(l / r),
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
        if !matches!(test, Value::Bool(false)) {
            eval(&expr.consequent, ctx)
        } else if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Ok(Value::None))
        }
    } else {
        Ok(Err(Failure()))
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

fn map_eval<F>(ctx: &mut EvalContext, expr: &Expression, op: F) -> EvalResult
where
    F: Fn(Value) -> Result<Value, EvalError>,
{
    match eval(expr, ctx) {
        Ok(Ok(v)) => op(v).map(|v| Ok(v)),
        t => t,
    }
}
