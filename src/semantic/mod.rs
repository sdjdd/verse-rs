use std::collections::HashMap;
use thiserror::Error;

use crate::{
    ast::*,
    core::{Symbol, SymbolTable},
    lexer::Span,
    runtime::FunctionId,
    semantic::{
        builtins::{BuiltinSymbols, BuiltinTypes},
        type_check::{TypeId, TypeInfo, TypeRegistry},
    },
};

pub mod builtins;
pub mod type_check;

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
    builtin_types: BuiltinTypes,
    pub errors: Vec<SemanticError>,
    void_functions: Vec<FunctionId>,
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
            (bs.s_void, TypeInfo::Void),
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

        let bt = BuiltinTypes::install(&mut root_scope.types);

        Self {
            expr_type: HashMap::new(),
            scopes: vec![root_scope],
            primitive_types,
            builtin_types: bt,
            errors: vec![],
            void_functions: vec![],
        }
    }

    pub fn get_void_functions(&self) -> &[FunctionId] {
        &self.void_functions
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

    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.scopes.last_mut().unwrap().types
    }

    pub fn lookup_type(&self, type_id: TypeId) -> Option<&TypeInfo> {
        for scope in self.scopes.iter().rev() {
            let v = scope.types.lookup(type_id);
            if v.is_some() {
                return v;
            }
        }
        None
    }

    pub fn resolve_type_expr(&mut self, type_expr: &TypeExpr) -> TypeId {
        match &type_expr.kind {
            TypeExprKind::Named(symbol) => {
                let scope = self.scopes.last().unwrap();
                self.primitive_types
                    .get(symbol)
                    .copied()
                    .or_else(|| scope.types.resolve(&TypeInfo::Named(*symbol)))
                    .unwrap_or(self.builtin_types.t_any)
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    let arg_id = self.resolve_type_expr(arg);
                    arg_ids.push(arg_id);
                }
                let scope = self.scopes.last_mut().unwrap();
                let type_id = scope.types.intern(TypeInfo::Tuple(arg_ids));
                type_id
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> =
                    params.iter().map(|p| self.resolve_type_expr(p)).collect();
                let return_type = self.resolve_type_expr(ret);
                let scope = self.scopes.last_mut().unwrap();
                let type_id = scope.types.intern(TypeInfo::Function {
                    params: param_types,
                    ret: return_type,
                });
                type_id
            }
        }
    }

    fn get_expr_type(&self, expr_id: ExprId) -> TypeId {
        self.expr_type
            .get(&expr_id)
            .cloned()
            .unwrap_or(self.builtin_types.t_any)
    }

    pub fn handle_expr(&mut self, expr: &Expression) {
        match &expr.kind {
            ExprKind::Integer(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_int);
            }
            ExprKind::Float(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_float);
            }
            ExprKind::Logic(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_logic);
            }
            ExprKind::Char(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_char);
            }
            ExprKind::Char32(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_char32);
            }
            ExprKind::String(_) => {
                self.expr_type.insert(expr.id, self.builtin_types.t_string);
            }
            ExprKind::Decl(e) => self.handle_decl_expr(expr, e),
            ExprKind::VarDecl(e) => self.handle_var_decl_expr(expr, e),
            ExprKind::Set(e) => self.handle_set_expr(expr, e),
            ExprKind::Id(e) => self.handle_id_expr(expr, e),
            ExprKind::Block(e) => self.handle_block_expr(expr, e),
            ExprKind::CompareChain(e) => self.handle_compare_chain_expr(expr, e),
            ExprKind::Template(e) => self.handle_template_expr(expr, e),
            ExprKind::Tuple(e) => self.handle_tuple_expr(expr, e),
            ExprKind::If(e) => self.handle_if_expr(expr, e),
            ExprKind::Func(e) => self.handle_func_expr(expr, e),
            ExprKind::Call(e) => self.handle_call_expr(expr, e),
            _ => unimplemented!(),
        }
    }

    fn handle_decl_expr(&mut self, outer: &Expression, expr: &DeclarationExpr) {
        self.handle_expr(&expr.value);
        let value_type = self.get_expr_type(expr.value.id);

        if let Some(typ) = &expr.typ {
            let decl_type = self.resolve_type_expr(typ);
            if decl_type != value_type {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.value.span.clone(),
                })
            }
            self.expr_type.insert(outer.id, decl_type);
        } else {
            self.expr_type.insert(outer.id, value_type);
        }

        self.declare(
            expr.target,
            Binding {
                type_id: value_type,
                mutable: false,
            },
        );
    }

    fn handle_var_decl_expr(&mut self, outer: &Expression, expr: &VarDeclExpr) {
        self.handle_expr(&expr.expr);

        let decl_type = self.resolve_type_expr(&expr.typ);
        let value_type = self.get_expr_type(expr.expr.id);

        if decl_type != value_type {
            self.errors.push(SemanticError::TypeMismatch {
                span: expr.expr.span.clone(),
            })
        }

        self.declare(
            expr.name.symbol,
            Binding {
                type_id: decl_type,
                mutable: true,
            },
        );

        self.expr_type.insert(outer.id, decl_type);
    }

    fn handle_set_expr(&mut self, outer: &Expression, expr: &SetExpr) {
        self.handle_expr(&expr.expr);
        let value_type = self.get_expr_type(expr.expr.id);
        let mut type_id = value_type;

        match &expr.target.kind {
            LValueKind::Id(id_expr) => {
                if let Some(binding) = self.loopup(&id_expr.symbol).cloned() {
                    type_id = binding.type_id;
                    if binding.type_id != value_type {
                        self.errors.push(SemanticError::TypeMismatch {
                            span: expr.expr.span.clone(),
                        })
                    }
                    if !binding.mutable {
                        self.errors.push(SemanticError::Mutability {
                            span: expr.target.span.clone(),
                            symbol: id_expr.symbol,
                        })
                    }
                }
            }
        }

        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_id_expr(&mut self, outer: &Expression, expr: &IdentifierExpr) {
        let type_id = if let Some(binding) = self.loopup(&expr.symbol) {
            binding.type_id
        } else {
            self.errors.push(SemanticError::Reference {
                span: outer.span.clone(),
                symbol: expr.symbol,
            });
            self.builtin_types.t_any
        };
        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_block_expr(&mut self, outer: &Expression, expr: &BlockExpr) {
        let mut type_id = self.builtin_types.t_void;
        for expr in &expr.body {
            self.handle_expr(expr);
            type_id = self.get_expr_type(expr.id);
        }
        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_compare_chain_expr(&mut self, outer: &Expression, expr: &CompareChainExpr) {
        // TODO: check if items are comparable
        // Currently just check if they are the same type
        self.handle_expr(&expr.head);
        let head_type = self.get_expr_type(expr.head.id);
        for (_, expr) in &expr.rest {
            self.handle_expr(expr);
            if self.get_expr_type(expr.id) != head_type {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.span.clone(),
                })
            }
        }
        self.expr_type.insert(outer.id, head_type);
    }

    fn handle_template_expr(&mut self, outer: &Expression, expr: &TemplateExpression) {
        for elem in &expr.elements {
            match elem {
                TemplateElement::Expr(expr) => self.handle_expr(expr),
                _ => {}
            }
        }
        self.expr_type.insert(outer.id, self.builtin_types.t_string);
    }

    fn handle_tuple_expr(&mut self, outer: &Expression, expr: &TupleExpr) {
        let mut elem_types = vec![];
        for elem in &expr.elements {
            self.handle_expr(elem);
            elem_types.push(self.get_expr_type(elem.id));
        }
        let type_id = self.type_registry_mut().intern(TypeInfo::Tuple(elem_types));
        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_if_expr(&mut self, outer: &Expression, expr: &IfExpr) {
        self.push_scope();
        self.handle_expr(&expr.test);
        self.handle_expr(&expr.consequent);
        self.pop_scope();

        let then_type = self.get_expr_type(expr.consequent.id);
        let mut type_id = then_type;

        if let Some(alt) = &expr.alternate {
            self.push_scope();
            self.handle_expr(alt);
            self.pop_scope();
            let else_type = self.get_expr_type(alt.id);
            if then_type != else_type {
                type_id = self.builtin_types.t_any;
            }
        } else {
            type_id = self.type_registry_mut().intern(TypeInfo::Option(type_id))
        }

        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_func_expr(&mut self, outer: &Expression, expr: &FunctionExpr) {
        let mut param_types = vec![];
        let return_type = self.resolve_type_expr(&expr.return_type);

        self.push_scope();
        for param in &expr.params {
            let param_type = self.resolve_type_expr(&param.typ);
            param_types.push(param_type);
            self.declare(
                param.name,
                Binding {
                    type_id: param_type,
                    mutable: true,
                },
            );
        }
        self.handle_expr(&expr.body);
        self.pop_scope();

        if return_type != self.builtin_types.t_void {
            if self.get_expr_type(expr.body.id) != return_type {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.body.span.clone(),
                })
            }
        } else {
            self.void_functions.push(FunctionId(outer.id.0));
        }

        let type_id = self.type_registry_mut().intern(TypeInfo::Function {
            params: param_types,
            ret: return_type,
        });
        self.declare(
            expr.name,
            Binding {
                type_id,
                mutable: false,
            },
        );

        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_call_expr(&mut self, outer: &Expression, expr: &CallExpr) {
        self.handle_expr(&expr.callee);
        let callee_type = self.get_expr_type(expr.callee.id);
        let mut return_type = self.builtin_types.t_any;

        if let Some(callee_type) = self.lookup_type(callee_type).cloned() {
            match callee_type {
                TypeInfo::Function { params, ret } => {
                    return_type = ret;
                    if params.len() != expr.args.len() {
                        self.errors.push(SemanticError::ArgsCountMismatch {
                            span: expr.callee.span.clone(),
                        })
                    }
                    for (&param_type, arg) in params.iter().zip(expr.args.iter()) {
                        self.handle_expr(arg);
                        let arg_type = self.get_expr_type(arg.id);
                        if param_type != arg_type {
                            self.errors.push(SemanticError::TypeMismatch {
                                span: arg.span.clone(),
                            })
                        }
                    }
                }
                TypeInfo::Any => {
                    // TODO: handle builtin functions
                }
                _ => {
                    self.errors.push(SemanticError::TypeMismatch {
                        span: expr.callee.span.clone(),
                    });
                }
            }
        }

        self.expr_type.insert(outer.id, return_type);
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

    #[error("{span:?} arguments count mismatch")]
    ArgsCountMismatch { span: Span },
}
