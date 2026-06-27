use std::collections::HashMap;

use crate::{
    ast::*,
    core::Symbol,
    semantic::{Binding, SemanticContext, SemanticError},
};

pub type SemanticCheckResult = Result<(), SemanticError>;

pub struct TypeCheckContext {}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct TypeId(usize);

#[derive(Hash, PartialEq, Eq, Clone)]
pub enum TypeInfo {
    Void,
    Any,
    Int,
    Float,
    Logic,
    Char,
    Char32,
    String,
    Tuple(Vec<TypeId>),
    Option(TypeId),
    Named(Symbol),
}

#[derive(Default)]
pub struct TypeRegistry {
    map: HashMap<TypeInfo, TypeId>,
    vec: Vec<TypeInfo>,
}

impl TypeRegistry {
    fn intern(&mut self, key: TypeInfo) -> TypeId {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }
        let id = TypeId(self.vec.len());
        self.map.insert(key.clone(), id);
        self.vec.push(key);
        id
    }
}

pub fn resolve_expr_type(expr: &Expression, ctx: &mut SemanticContext) -> TypeId {
    let type_id = match &expr.kind {
        ExprKind::Integer(_) => ctx.type_registry.intern(TypeInfo::Int),
        ExprKind::Float(_) => ctx.type_registry.intern(TypeInfo::Float),
        ExprKind::Logic(_) => ctx.type_registry.intern(TypeInfo::Logic),
        ExprKind::Char(_) => ctx.type_registry.intern(TypeInfo::Char),
        ExprKind::Char32(_) => ctx.type_registry.intern(TypeInfo::Char32),
        ExprKind::String(_) | ExprKind::Template(_) => ctx.type_registry.intern(TypeInfo::String),
        ExprKind::Tuple(expr) => {
            let key = TypeInfo::Tuple(
                expr.elements
                    .iter()
                    .map(|el| resolve_expr_type(el, ctx))
                    .collect(),
            );
            ctx.type_registry.intern(key)
        }
        // TODO: implement this
        ExprKind::Call(_) => ctx.type_registry.intern(TypeInfo::Void),
        ExprKind::Block(expr) => {
            if let Some(last_expr) = expr.body.last() {
                resolve_expr_type(last_expr, ctx)
            } else {
                ctx.type_registry.intern(TypeInfo::Void)
            }
        }
        ExprKind::If(expr) => {
            let conseq_type = resolve_expr_type(&expr.consequent, ctx);
            if let Some(alt) = &expr.alternate {
                let alt_type = resolve_expr_type(alt, ctx);
                if conseq_type == alt_type {
                    conseq_type
                } else {
                    ctx.type_registry.intern(TypeInfo::Any)
                }
            } else {
                ctx.type_registry.intern(TypeInfo::Option(conseq_type))
            }
        }
        ExprKind::CompareChain(expr) => resolve_expr_type(&expr.head, ctx),
        ExprKind::Decl(expr) => {
            let type_id = resolve_expr_type(&expr.value, ctx);
            match &expr.target.kind {
                LValueKind::Id(id) => ctx.bindings.insert(
                    id.symbol,
                    Binding {
                        type_id,
                        mutable: false,
                    },
                ),
            };
            type_id
        }
        ExprKind::VarDecl(expr) => {
            let type_id = resolve_expr_type(&expr.expr, ctx);
            ctx.bindings.insert(
                expr.name.symbol,
                Binding {
                    type_id,
                    mutable: true,
                },
            );
            type_id
        }
        ExprKind::Id(expr) => ctx.bindings.get(&expr.symbol).unwrap().type_id,
        ExprKind::Set(e) => {
            resolve_expr_type(&e.expr, ctx);
            ctx.type_registry.intern(TypeInfo::Void)
        }
        _ => unimplemented!("{:?}", expr.kind),
    };
    ctx.expr_type.insert(expr.id, type_id);
    type_id
}

pub fn check_expr(expr: &Expression, ctx: &mut SemanticContext) -> SemanticCheckResult {
    match &expr.kind {
        ExprKind::Set(e) => {
            let lvalue = match &e.target.kind {
                LValueKind::Id(id) => {
                    let binding = ctx.bindings.get(&id.symbol).unwrap();
                    if !binding.mutable {
                        return Err(SemanticError::Mutability {
                            span: expr.span.clone(),
                            symbol: id.symbol,
                        });
                    }
                    binding
                }
            };
            if lvalue.type_id != *ctx.expr_type.get(&e.expr.id).unwrap() {
                Err(SemanticError::TypeMismatch {
                    span: expr.span.clone(),
                })
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}
