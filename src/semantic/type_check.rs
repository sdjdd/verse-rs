use std::collections::HashMap;

use crate::{
    ast::*,
    core::Symbol,
    lexer::Span,
    semantic::{Binding, SemanticContext, SemanticError},
};

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
    alias: HashMap<TypeId, TypeId>,
}

impl TypeRegistry {
    pub fn intern(&mut self, key: TypeInfo) -> TypeId {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }

        let key = match &key {
            TypeInfo::Tuple(element_ids) => {
                TypeInfo::Tuple(element_ids.iter().map(|&id| self.resolve_id(id)).collect())
            }
            _ => key,
        };

        let id = TypeId(self.vec.len());
        self.map.insert(key.clone(), id);
        self.vec.push(key);
        id
    }

    pub fn set_alias(&mut self, src: TypeId, dst: TypeId) {
        self.alias.insert(dst, src);
    }

    pub fn resolve_id(&self, mut id: TypeId) -> TypeId {
        loop {
            if let Some(&src_id) = self.alias.get(&id) {
                id = src_id
            } else {
                break id;
            }
        }
    }

    pub fn resolve(&self, type_info: &TypeInfo) -> Option<TypeId> {
        if let Some(&id) = self.map.get(type_info) {
            Some(self.resolve_id(id))
        } else {
            None
        }
    }
}

pub fn resolve_expr_type(
    expr: &Expression,
    ctx: &mut SemanticContext,
) -> Result<TypeId, SemanticError> {
    let type_id = match &expr.kind {
        ExprKind::Integer(_) => ctx.type_registry_mut().intern(TypeInfo::Int),
        ExprKind::Float(_) => ctx.type_registry_mut().intern(TypeInfo::Float),
        ExprKind::Logic(_) => ctx.type_registry_mut().intern(TypeInfo::Logic),
        ExprKind::Char(_) => ctx.type_registry_mut().intern(TypeInfo::Char),
        ExprKind::Char32(_) => ctx.type_registry_mut().intern(TypeInfo::Char32),
        ExprKind::String(_) => ctx.type_registry_mut().intern(TypeInfo::String),
        ExprKind::Template(expr) => handle_template_expr(expr, ctx)?,
        ExprKind::Tuple(expr) => handle_tuple_expr(expr, ctx)?,
        ExprKind::Call(expr) => handle_call_expr(expr, ctx)?,
        ExprKind::Block(expr) => handle_block_expr(expr, ctx)?,
        ExprKind::If(expr) => handle_if_expr(expr, ctx)?,
        ExprKind::CompareChain(expr) => resolve_expr_type(&expr.head, ctx)?,
        ExprKind::Decl(e) => handle_decl_expr(e, ctx)?,
        ExprKind::VarDecl(expr) => handle_var_decl_expr(expr, ctx)?,
        ExprKind::Id(e) => handle_id_expr(e, ctx, &expr.span)?,
        ExprKind::Set(e) => handle_set_expr(e, ctx, &expr.span)?,
        _ => unimplemented!("{:?}", expr.kind),
    };
    ctx.expr_type.insert(expr.id, type_id);
    Ok(type_id)
}

fn handle_template_expr(
    expr: &TemplateExpression,
    ctx: &mut SemanticContext,
) -> Result<TypeId, SemanticError> {
    for elem in &expr.elements {
        match elem {
            TemplateElement::Expr(texpr) => {
                resolve_expr_type(texpr, ctx)?;
            }
            _ => {}
        }
    }
    Ok(ctx.type_registry_mut().intern(TypeInfo::String))
}

fn handle_tuple_expr(expr: &TupleExpr, ctx: &mut SemanticContext) -> Result<TypeId, SemanticError> {
    let elements: Result<Vec<_>, _> = expr
        .elements
        .iter()
        .map(|el| resolve_expr_type(el, ctx))
        .collect();
    let key = TypeInfo::Tuple(elements?);
    Ok(ctx.type_registry_mut().intern(key))
}

fn handle_call_expr(expr: &CallExpr, ctx: &mut SemanticContext) -> Result<TypeId, SemanticError> {
    resolve_expr_type(&expr.callee, ctx)?;
    for arg in &expr.args {
        resolve_expr_type(arg, ctx)?;
    }
    Ok(ctx.type_registry_mut().intern(TypeInfo::Void))
}

fn handle_block_expr(expr: &BlockExpr, ctx: &mut SemanticContext) -> Result<TypeId, SemanticError> {
    ctx.push_scope();
    let mut type_id = None;
    for body_expr in &expr.body {
        type_id = Some(resolve_expr_type(body_expr, ctx)?);
    }
    ctx.pop_scope();
    Ok(if type_id.is_some() {
        type_id.unwrap()
    } else {
        ctx.type_registry_mut().intern(TypeInfo::Void)
    })
}

fn handle_if_expr(expr: &IfExpr, ctx: &mut SemanticContext) -> Result<TypeId, SemanticError> {
    ctx.push_scope();
    resolve_expr_type(&expr.test, ctx)?;
    let then_type = resolve_expr_type(&expr.consequent, ctx)?;
    ctx.pop_scope();
    let type_id = if let Some(else_expr) = &expr.alternate {
        if then_type == resolve_expr_type(else_expr, ctx)? {
            then_type
        } else {
            ctx.type_registry_mut().intern(TypeInfo::Any)
        }
    } else {
        ctx.type_registry_mut().intern(TypeInfo::Option(then_type))
    };
    Ok(type_id)
}

fn handle_decl_expr(
    expr: &DeclarationExpr,
    ctx: &mut SemanticContext,
) -> Result<TypeId, SemanticError> {
    let type_id = resolve_expr_type(&expr.value, ctx)?;
    match &expr.target.kind {
        LValueKind::Id(id) => ctx.declare(
            id.symbol,
            Binding {
                type_id,
                mutable: false,
            },
        ),
    };

    if let Some(typ) = &expr.typ {
        let decl_type_id = ctx.resolve_type_expr(typ)?;
        if decl_type_id != type_id {
            return Err(SemanticError::TypeMismatch {
                span: expr.value.span.clone(),
            });
        }
    }

    Ok(type_id)
}

fn handle_var_decl_expr(
    expr: &VarDeclExpr,
    ctx: &mut SemanticContext,
) -> Result<TypeId, SemanticError> {
    let type_id = resolve_expr_type(&expr.expr, ctx)?;
    let decl_type_id = ctx.resolve_type_expr(&expr.typ)?;

    if decl_type_id != type_id {
        return Err(SemanticError::TypeMismatch {
            span: expr.expr.span.clone(),
        });
    }

    ctx.declare(
        expr.name.symbol,
        Binding {
            type_id,
            mutable: true,
        },
    );

    Ok(type_id)
}

fn handle_id_expr(
    expr: &IdentifierExpr,
    ctx: &mut SemanticContext,
    span: &Span,
) -> Result<TypeId, SemanticError> {
    let type_id = if let Some(binding) = ctx.loopup(&expr.symbol) {
        binding.type_id
    } else {
        return Err(SemanticError::Reference {
            span: span.clone(),
            symbol: expr.symbol,
        });
    };
    Ok(type_id)
}

fn handle_set_expr(
    expr: &SetExpr,
    ctx: &mut SemanticContext,
    span: &Span,
) -> Result<TypeId, SemanticError> {
    resolve_expr_type(&expr.expr, ctx)?;

    let value_type = *ctx.expr_type.get(&expr.expr.id).unwrap();

    let lvalue = match &expr.target.kind {
        LValueKind::Id(id) => {
            handle_id_expr(id, ctx, &expr.target.span)?;
            let binding = ctx.loopup(&id.symbol).unwrap();
            if !binding.mutable {
                return Err(SemanticError::Mutability {
                    span: span.clone(),
                    symbol: id.symbol,
                });
            }
            binding
        }
    };

    if lvalue.type_id != value_type {
        return Err(SemanticError::TypeMismatch {
            span: expr.expr.span.clone(),
        });
    }

    Ok(ctx.type_registry_mut().intern(TypeInfo::Void))
}
