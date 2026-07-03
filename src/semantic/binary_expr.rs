use crate::{
    ast::BinaryExpr,
    ir,
    lexer::Span,
    semantic::{AnalysisResult, SemanticAnalyzer},
};

impl SemanticAnalyzer {
    pub(super) fn handle_binary_expr(&mut self, span: Span, expr: &BinaryExpr) -> AnalysisResult {
        let lhs_ar = self.handle_expr(&expr.lhs);
        let rhs_ar = self.handle_expr(&expr.rhs);
        let lhs_type = lhs_ar.expr_type;
        let rhs_type = rhs_ar.expr_type;

        let type_id = if lhs_type == rhs_type {
            lhs_type
        } else {
            self.emit_type_mismatch_error(span.clone(), lhs_type, rhs_type);
            self.builtin_types.t_any
        };

        AnalysisResult {
            expr_type: type_id,
            ir_id: if let (Some(lhs_ir), Some(rhs_ir)) = (lhs_ar.ir_id, rhs_ar.ir_id) {
                Some(self.emit_ir(
                    ir::ExprKind::Binary(ir::BinaryExpr {
                        lhs: lhs_ir,
                        op: expr.op,
                        rhs: rhs_ir,
                    }),
                    type_id,
                ))
            } else {
                None
            },
        }
    }
}
