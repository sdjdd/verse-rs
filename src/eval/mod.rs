use std::{collections::HashMap, fmt::Display};

use thiserror::Error;

use crate::ast::{
    AssignmentExpr, BinaryExpr, BinaryOperator, CallExpr, Expression, IdentifierExpr, IfExpr,
    LiteralExpr, TemplateElement, TemplateExpression,
};

#[derive(Clone, Debug)]
pub enum Value {
    None,
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    Bool(bool),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::None => write!(f, ""),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Integer(value) => write!(f, "{}", value),
            Value::Float(value) => write!(f, "{}", value),
            Value::Char(value) => write!(f, "{}", *value as char),
            Value::Char32(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Debug)]
pub struct Failure();

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("ReferenceError: {0}")]
    ReferenceError(String),
}

pub type EvalResult = Result<Result<Value, Failure>, EvalError>;

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
    match expr.callee.as_str() {
        "Print" => {
            if let Some(arg) = expr.arguments.first() {
                println!("{}", eval(arg, ctx)?.unwrap());
            }
        }
        _ => unimplemented!(),
    };
    Ok(Ok(Value::None))
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
                BinaryOperator::Eq => match (left, right) {
                    (Value::Integer(l), Value::Integer(r)) => Value::Bool(l == r),
                    (Value::Float(l), Value::Float(r)) => Value::Bool(l == r),
                    _ => unimplemented!(),
                },
            };
            Ok(Ok(value))
        }
        _ => Ok(Err(Failure())),
    }
}

fn eval_if(expr: &IfExpr, ctx: &mut EvalContext) -> EvalResult {
    let test = eval(&expr.test, ctx)?;
    if let Ok(test) = test {
        if !matches!(test, Value::Bool(false)) {
            eval(&expr.consequent, ctx)
        } else if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Ok(Value::None))
        }
    } else {
        if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Ok(Value::None))
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
