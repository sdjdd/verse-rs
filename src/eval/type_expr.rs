use crate::{
    ast::TypeExpr,
    eval::Evaluator,
    runtime::{Failure, Value},
};

impl Evaluator {
    pub(super) fn eval_type_expr(&mut self, expr: &TypeExpr) -> Result<Value, Failure> {
        let type_id = self.expr_types.get(&expr.id).unwrap();
        Ok(Value::Type(*type_id))
    }
}
