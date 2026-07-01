use crate::{
    ast::{BinaryExpr, Expression},
    semantic::{SemanticContext, SemanticError},
};

impl SemanticContext {
    pub(super) fn handle_binary_expr(&mut self, outer: &Expression, expr: &BinaryExpr) {
        self.handle_expr(&expr.lhs);
        self.handle_expr(&expr.rhs);
        let lhs_type = self.get_expr_type(expr.lhs.id);
        let rhs_type = self.get_expr_type(expr.rhs.id);

        if lhs_type == rhs_type {
            self.expr_type.insert(outer.id, lhs_type);
        } else {
            self.errors.push(SemanticError::TypeMismatch {
                span: outer.span.clone(),
                expect: lhs_type,
                found: rhs_type,
            });
            self.expr_type.insert(outer.id, self.builtin_types.t_any);
        }
    }
}
