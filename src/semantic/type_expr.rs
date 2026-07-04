use crate::{
    ast::{TypeExpr, TypeExprKind},
    runtime::TypeId,
    semantic::{SemanticAnalyzer, SemanticError, TypeInfo},
};

impl SemanticAnalyzer {
    pub(super) fn handle_type_expr(&mut self, expr: &TypeExpr) -> TypeId {
        let type_id = match &expr.kind {
            TypeExprKind::Named(symbol) => (|| -> TypeId {
                if let Some(binding) = self.lookup(symbol) {
                    let type_id = binding.type_id;
                    if let Some(ty) = self.types.lookup(type_id) {
                        if let TypeInfo::Type(inner_type) = ty {
                            return *inner_type;
                        } else {
                            self.errors.push(SemanticError::UnexpectedExpr {
                                span: expr.span.clone(),
                                expect: "type".to_string(),
                                found: "value".to_string(),
                            });
                            return self.builtin_types.t_any;
                        }
                    }
                }
                self.errors.push(SemanticError::TypeNotFound {
                    span: expr.span.clone(),
                    symbol: *symbol,
                });
                self.builtin_types.t_any
            })(),
            TypeExprKind::Option(inner) => {
                let inner = self.handle_type_expr(inner);
                self.types.intern(TypeInfo::Option(inner))
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    arg_ids.push(self.handle_type_expr(arg));
                }
                self.types.intern(TypeInfo::Tuple(arg_ids))
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> = params.iter().map(|p| self.handle_type_expr(p)).collect();
                let ret_ty = self.handle_type_expr(ret);
                let inner_type = self.types.intern(TypeInfo::Function {
                    params: param_types,
                    ret: ret_ty,
                });
                self.types.intern(TypeInfo::Type(inner_type))
            }
            TypeExprKind::Type => {
                panic!("{:?}", expr);
                // self.builtin_types.t_any
            }
        };

        type_id
    }
}
