use crate::{
    ast::{BinaryExpr, Expression},
    semantic::SemanticAnalyzer,
};

impl SemanticAnalyzer {
    pub(super) fn handle_binary_expr(&mut self, outer: &Expression, expr: &BinaryExpr) {
        self.handle_expr(&expr.lhs);
        self.handle_expr(&expr.rhs);
        let lhs_type = self.get_expr_type(expr.lhs.id);
        let rhs_type = self.get_expr_type(expr.rhs.id);

        if lhs_type == rhs_type {
            self.expr_type.insert(outer.id, lhs_type);
        } else {
            self.emit_type_mismatch_error(outer.span.clone(), lhs_type, rhs_type);
            self.expr_type.insert(outer.id, self.builtin_types.t_any);
        }
    }
}
