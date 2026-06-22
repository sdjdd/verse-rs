use std::{collections::HashMap, fmt::Display};

use thiserror::Error;

use crate::ast::{
    AssignmentExpr, BinaryExpr, BinaryOperator, CallExpr, Expression, IdentifierExpr, IfExpr,
    LiteralExpr,
};

#[derive(Clone, Copy, Debug)]
pub enum Value {
    None,
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
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
        }
    }
}

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("ReferenceError: {0}")]
    ReferenceError(String),

    #[error("TypeError: {0}")]
    TypeError(String),
}

pub type EvalResult = Result<Value, EvalError>;

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
    }
}

fn eval_assignment(expr: &AssignmentExpr, ctx: &mut EvalContext) -> EvalResult {
    let value = eval(&expr.expr, ctx)?;
    ctx.bindings.insert(expr.target.clone(), value);
    Ok(value)
}

fn eval_identifier(expr: &IdentifierExpr, ctx: &mut EvalContext) -> EvalResult {
    if let Some(value) = ctx.bindings.get(&expr.name) {
        Ok(*value)
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
        LiteralExpr::Bool(value) => Value::Bool(*value),
    };
    Ok(value)
}

fn eval_call(expr: &CallExpr, ctx: &mut EvalContext) -> EvalResult {
    match expr.callee.as_str() {
        "Print" => {
            if let Some(arg) = expr.arguments.first() {
                println!("{}", eval(arg, ctx)?);
            }
        }
        _ => unimplemented!(),
    };
    Ok(Value::None)
}

fn eval_binary(expr: &BinaryExpr, ctx: &mut EvalContext) -> EvalResult {
    let left = eval(&expr.left, ctx)?;
    let right = eval(&expr.right, ctx)?;
    let value = match expr.operator {
        BinaryOperator::Plus => match (left, right) {
            (Value::Integer(l), Value::Integer(r)) => Value::Integer(l + r),
            (Value::Float(l), Value::Float(r)) => Value::Float(l + r),
            _ => unimplemented!("{:?} op {:?}", left, right),
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
    Ok(value)
}

fn eval_if(expr: &IfExpr, ctx: &mut EvalContext) -> EvalResult {
    let test = eval(&expr.test, ctx)?;
    if let Value::Bool(test) = test {
        if test {
            eval(&expr.consequent, ctx)
        } else if let Some(alternate) = &expr.alternate {
            eval(alternate, ctx)
        } else {
            Ok(Value::None)
        }
    } else {
        Err(EvalError::TypeError(
            "Expected bool in if condition".to_string(),
        ))
    }
}
