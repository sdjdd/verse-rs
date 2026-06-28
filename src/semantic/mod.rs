use std::collections::HashMap;
use thiserror::Error;

use crate::{
    ast::{ExprId, TypeExpr, TypeExprKind},
    core::{Symbol, SymbolTable},
    lexer::Span,
    semantic::{
        builtins::BuiltinSymbols,
        type_check::{TypeId, TypeInfo, TypeRegistry},
    },
};

pub mod builtins;
pub mod type_check;

pub use type_check::resolve_expr_type;

#[derive(Clone, Copy)]
pub struct Binding {
    pub type_id: TypeId,
    pub mutable: bool,
}

#[derive(Default)]
pub struct Scope {
    types: TypeRegistry,
    bindings: HashMap<Symbol, Binding>,
}

pub struct SemanticContext {
    expr_type: HashMap<ExprId, TypeId>,
    scopes: Vec<Scope>,
    primitive_types: HashMap<Symbol, TypeId>,
}

impl SemanticContext {
    pub fn new(symbol_table: &mut SymbolTable) -> Self {
        let mut root_scope = Scope::default();
        let mut primitive_types = HashMap::new();

        let bs = BuiltinSymbols::install(symbol_table);
        let primitive_type_map = [
            (bs.s_int, TypeInfo::Int),
            (bs.s_float, TypeInfo::Float),
            (bs.s_char, TypeInfo::Char),
            (bs.s_char32, TypeInfo::Char32),
            (bs.s_logic, TypeInfo::Logic),
            (bs.s_string, TypeInfo::String),
        ];

        for (symbol, type_info) in primitive_type_map {
            primitive_types.insert(symbol, root_scope.types.intern(type_info));
        }

        root_scope.bindings.insert(
            bs.s_print,
            Binding {
                type_id: root_scope.types.intern(TypeInfo::Any),
                mutable: false,
            },
        );

        Self {
            expr_type: HashMap::new(),
            scopes: vec![root_scope],
            primitive_types,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, symbol: Symbol, binding: Binding) {
        self.scopes
            .last_mut()
            .unwrap()
            .bindings
            .insert(symbol, binding);
    }

    fn loopup(&mut self, symbol: &Symbol) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            let binding = scope.bindings.get(symbol);
            if binding.is_some() {
                return binding;
            }
        }
        None
    }

    pub fn type_registry(&self) -> &TypeRegistry {
        &self.scopes.last().unwrap().types
    }

    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.scopes.last_mut().unwrap().types
    }

    pub fn resolve_type_expr(&mut self, type_expr: &TypeExpr) -> Result<TypeId, SemanticError> {
        match &type_expr.kind {
            TypeExprKind::Named(symbol) => {
                let scope = self.scopes.last().unwrap();
                self.primitive_types
                    .get(symbol)
                    .copied()
                    .or_else(|| scope.types.resolve(&TypeInfo::Named(*symbol)))
                    .ok_or_else(|| SemanticError::TypeNotFound {
                        span: type_expr.span.clone(),
                        symbol: *symbol,
                    })
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    let arg_id = self.resolve_type_expr(arg)?;
                    arg_ids.push(arg_id);
                }
                let scope = self.scopes.last_mut().unwrap();
                let type_id = scope.types.intern(TypeInfo::Tuple(arg_ids));
                Ok(type_id)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum SemanticError {
    #[error("{span:?}: cannot mutate immutable {symbol:?}")]
    Mutability { span: Span, symbol: Symbol },

    #[error("{span:?} mismatched types")]
    TypeMismatch { span: Span },

    #[error("{span:?} cannot resolve value {symbol:?}")]
    Reference { span: Span, symbol: Symbol },

    #[error("{span:?} cannot resolve type {symbol:?}")]
    TypeNotFound { span: Span, symbol: Symbol },
}
