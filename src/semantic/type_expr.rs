use crate::{
    ast::{TypeExpr, TypeExprKind},
    semantic::{SemanticAnalyzer, SemanticError, TypeInfo},
};

impl SemanticAnalyzer {
    pub(super) fn handle_type_expr(&mut self, expr: &TypeExpr) {
        let type_id = match &expr.kind {
            TypeExprKind::Named(symbol) => {
                self.lookup_type_by_symbol(*symbol).unwrap_or_else(|| {
                    self.errors.push(SemanticError::TypeNotFound {
                        span: expr.span.clone(),
                        symbol: *symbol,
                    });
                    self.builtin_types.t_any
                })
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    self.handle_type_expr(arg);
                    arg_ids.push(self.get_expr_type(arg.id));
                }
                let inner_type = self.types.intern(TypeInfo::Tuple(arg_ids));
                self.types.intern(TypeInfo::Type(inner_type))
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> = params
                    .iter()
                    .map(|p| {
                        self.handle_type_expr(p);
                        self.get_expr_type(p.id)
                    })
                    .collect();
                self.handle_type_expr(ret);
                let inner_type = self.types.intern(TypeInfo::Function {
                    params: param_types,
                    ret: self.get_expr_type(ret.id),
                });
                self.types.intern(TypeInfo::Type(inner_type))
            }
            TypeExprKind::Type => self.builtin_types.t_any,
        };

        self.expr_type.insert(expr.id, type_id);
    }
}
