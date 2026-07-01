use std::collections::HashMap;
use thiserror::Error;

use crate::{
    ast::*,
    core::{Symbol, SymbolTable},
    lexer::Span,
    runtime::{FunctionId, TypeId},
    semantic::builtins::{BuiltinSymbols, BuiltinTypes},
};

pub mod builtins;

mod binary_expr;

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
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
    Function { params: Vec<TypeId>, ret: TypeId },
    Option(TypeId),

    Type(TypeId),
}

#[derive(Default, Debug)]
pub struct TypeRegistry {
    map: HashMap<TypeInfo, TypeId>,
    vec: Vec<TypeInfo>,
}

impl TypeRegistry {
    pub fn intern(&mut self, key: TypeInfo) -> TypeId {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }

        let id = TypeId(self.vec.len());
        self.map.insert(key.clone(), id);
        self.vec.push(key);
        id
    }

    fn lookup(&self, type_id: TypeId) -> Option<&TypeInfo> {
        self.vec.get(type_id.0)
    }
}

#[derive(Clone, Copy)]
pub struct Binding {
    pub type_id: TypeId,
    pub mutable: bool,
}

#[derive(Default)]
pub struct Scope {
    bindings: HashMap<Symbol, Binding>,
    types: HashMap<Symbol, TypeId>,
}

pub struct SemanticAnalyzer {
    expr_type: HashMap<ExprId, TypeId>,
    scopes: Vec<Scope>,
    pub builtin_symbols: BuiltinSymbols,
    pub builtin_types: BuiltinTypes,
    pub errors: Vec<SemanticError>,
    void_functions: Vec<FunctionId>,
    types: TypeRegistry,
}

impl SemanticAnalyzer {
    pub fn new(symbol_table: &mut SymbolTable) -> Self {
        let mut root_scope = Scope::default();
        let mut types = TypeRegistry::default();

        let bs = BuiltinSymbols::install(symbol_table);
        let bt = BuiltinTypes::install(&mut types);

        for (symbol, type_id) in bt.pairs(&bs) {
            root_scope.bindings.insert(
                symbol,
                Binding {
                    type_id: types.intern(TypeInfo::Type(type_id)),
                    mutable: false,
                },
            );
        }

        root_scope.bindings.insert(
            bs.s_print,
            Binding {
                type_id: types.intern(TypeInfo::Any),
                mutable: false,
            },
        );

        Self {
            expr_type: HashMap::new(),
            scopes: vec![root_scope],
            builtin_symbols: bs,
            builtin_types: bt,
            errors: vec![],
            void_functions: vec![],
            types,
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

    fn lookup(&mut self, symbol: &Symbol) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            let binding = scope.bindings.get(symbol);
            if binding.is_some() {
                return binding;
            }
        }
        None
    }

    fn declare_type(&mut self, symbol: Symbol, type_id: TypeId) {
        self.scopes
            .last_mut()
            .unwrap()
            .types
            .insert(symbol, type_id);
    }

    pub fn lookup_type(&self, type_id: TypeId) -> &TypeInfo {
        self.types
            .lookup(type_id)
            .expect("SemanticAnalyzer is not correctly setup or type_id is invalid.")
    }

    pub fn lookup_type_by_symbol(&self, symbol: Symbol) -> Option<TypeId> {
        self.scopes
            .iter()
            .rev()
            .map(|scope| scope.types.get(&symbol).copied())
            .find(|v| v.is_some())
            .flatten()
    }

    fn is_assignable_to(&self, from: TypeId, to: TypeId) -> bool {
        if from == to || to == self.builtin_types.t_any {
            return true;
        }

        false
    }

    pub fn intern_type_expr(&mut self, type_expr: &TypeExpr) -> TypeId {
        match &type_expr.kind {
            TypeExprKind::Named(symbol) => {
                self.lookup_type_by_symbol(*symbol).unwrap_or_else(|| {
                    self.errors.push(SemanticError::TypeNotFound {
                        span: type_expr.span.clone(),
                        symbol: *symbol,
                    });
                    self.builtin_types.t_any
                })
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    let arg_id = self.intern_type_expr(arg);
                    arg_ids.push(arg_id);
                }
                self.types.intern(TypeInfo::Tuple(arg_ids))
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> = params.iter().map(|p| self.intern_type_expr(p)).collect();
                let return_type = self.intern_type_expr(ret);
                self.types.intern(TypeInfo::Function {
                    params: param_types,
                    ret: return_type,
                })
            }
            TypeExprKind::Type => self.builtin_types.t_any,
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
            ExprKind::Binary(e) => self.handle_binary_expr(expr, e),
        }
    }

    fn handle_decl_expr(&mut self, outer: &Expression, expr: &DeclExpr) {
        self.handle_expr(&expr.value);
        let value_type = self.get_expr_type(expr.value.id);

        let binding_type = if let Some(typ) = &expr.typ {
            let decl_type = self.intern_type_expr(typ);
            if !self.is_assignable_to(value_type, decl_type) {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.value.span.clone(),
                    expect: decl_type,
                    found: value_type,
                })
            }
            decl_type
        } else {
            value_type
        };

        self.expr_type.insert(outer.id, binding_type);

        let binding = Binding {
            type_id: binding_type,
            mutable: false,
        };
        self.declare(expr.target, binding);

        if let TypeInfo::Type(inner_type) = self.lookup_type(binding_type) {
            self.declare_type(expr.target, *inner_type);
        }
    }

    fn handle_var_decl_expr(&mut self, outer: &Expression, expr: &VarDeclExpr) {
        self.handle_expr(&expr.expr);

        let decl_type = self.intern_type_expr(&expr.typ);
        let value_type = self.get_expr_type(expr.expr.id);

        if !self.is_assignable_to(value_type, decl_type) {
            self.errors.push(SemanticError::TypeMismatch {
                span: expr.expr.span.clone(),
                expect: decl_type,
                found: value_type,
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
                if let Some(binding) = self.lookup(&id_expr.symbol).cloned() {
                    type_id = binding.type_id;
                    if binding.type_id != value_type {
                        self.errors.push(SemanticError::TypeMismatch {
                            span: expr.expr.span.clone(),
                            expect: binding.type_id,
                            found: value_type,
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

    fn handle_id_expr(&mut self, outer: &Expression, expr: &IdExpr) {
        let type_id = if let Some(binding) = self.lookup(&expr.symbol).cloned() {
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
            let item_type = self.get_expr_type(expr.id);
            if item_type != head_type {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.span.clone(),
                    expect: head_type,
                    found: item_type,
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
        let type_id = self.types.intern(TypeInfo::Tuple(elem_types));
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
            type_id = self.types.intern(TypeInfo::Option(type_id))
        }

        self.expr_type.insert(outer.id, type_id);
    }

    fn handle_func_expr(&mut self, outer: &Expression, expr: &FunctionExpr) {
        let mut param_types = vec![];
        let return_type = self.intern_type_expr(&expr.return_type);

        self.push_scope();
        for param in &expr.params {
            let param_type = self.intern_type_expr(&param.typ);
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
            let body_type = self.get_expr_type(expr.body.id);
            if body_type != return_type {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.body.span.clone(),
                    expect: return_type,
                    found: body_type,
                })
            }
        } else {
            self.void_functions.push(FunctionId(outer.id.0));
        }

        let type_id = self.types.intern(TypeInfo::Function {
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

        match self.lookup_type(callee_type).clone() {
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
                    if !self.is_assignable_to(arg_type, param_type) {
                        self.errors.push(SemanticError::TypeMismatch {
                            span: arg.span.clone(),
                            expect: param_type,
                            found: arg_type,
                        })
                    }
                }
            }
            TypeInfo::Any => {
                // TODO: handle builtin functions
            }
            TypeInfo::Tuple(elements) => {
                if expr.args.len() == 1 {
                    self.handle_expr(&expr.args[0]);
                    if let ExprKind::Integer(index) = expr.args[0].kind {
                        return_type = elements[index as usize];
                    } else {
                        self.errors.push(SemanticError::UnexpectedExpr {
                            span: expr.args[0].span.clone(),
                            expect: "integer".to_string(),
                            found: format!("{:?}", expr.args[0]),
                        });
                    }
                } else {
                    self.errors.push(SemanticError::ArgsCountMismatch {
                        span: outer.span.clone(),
                    });
                }
            }
            _ => {
                panic!("not callable")
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
    TypeMismatch {
        span: Span,
        expect: TypeId,
        found: TypeId,
    },

    #[error("{span:?} cannot resolve value {symbol:?}")]
    Reference { span: Span, symbol: Symbol },

    #[error("{span:?} cannot resolve type {symbol:?}")]
    TypeNotFound { span: Span, symbol: Symbol },

    #[error("{span:?} arguments count mismatch")]
    ArgsCountMismatch { span: Span },

    #[error("{span:?} unexpected expression")]
    UnexpectedExpr {
        span: Span,
        expect: String,
        found: String,
    },
}
