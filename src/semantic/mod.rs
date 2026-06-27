pub mod type_check;

use std::collections::HashMap;

use crate::{
    ast::ExprId,
    core::Symbol,
    lexer::Span,
    semantic::type_check::{TypeId, TypeRegistry},
};

use thiserror::Error;
pub use type_check::check_expr;
pub use type_check::resolve_expr_type;

#[derive(Clone, Copy)]
struct Binding {
    type_id: TypeId,
    mutable: bool,
}

#[derive(Default)]
pub struct SemanticContext {
    expr_type: HashMap<ExprId, TypeId>,
    type_registry: TypeRegistry,
    bindings: HashMap<Symbol, Binding>,
}

impl SemanticContext {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Error, Debug)]
pub enum SemanticError {
    #[error("{span:?}: cannot mutate immutable {symbol:?}")]
    Mutability { span: Span, symbol: Symbol },

    #[error("{span:?} mismatched types")]
    TypeMismatch { span: Span },
}
