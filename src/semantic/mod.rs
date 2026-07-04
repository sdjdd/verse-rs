use std::collections::HashMap;
use thiserror::Error;

use crate::{
    ast::*,
    core::{Symbol, SymbolTable},
    ir::{self, Ir},
    lexer::Span,
    runtime::TypeId,
    semantic::builtins::{BuiltinSymbols, BuiltinTypes},
};

pub mod builtins;

mod binary_expr;
mod type_expr;

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

#[derive(Default, Debug, Clone)]
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

    pub fn lookup(&self, type_id: TypeId) -> Option<&TypeInfo> {
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
}

#[derive(Clone, Copy)]
pub struct AnalysisResult {
    pub expr_type: TypeId,
    pub ir_id: Option<ir::ExprId>,
}

pub struct SemanticAnalyzer {
    pub builtin_symbols: BuiltinSymbols,
    pub builtin_types: BuiltinTypes,
    pub errors: Vec<SemanticError>,
    pub irs: Vec<Ir>,

    scopes: Vec<Scope>,
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
            bs.s_Print,
            Binding {
                type_id: types.intern(TypeInfo::Any),
                mutable: false,
            },
        );

        Self {
            scopes: vec![root_scope],
            builtin_symbols: bs,
            builtin_types: bt,
            errors: vec![],
            types,
            irs: vec![],
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

    fn lookup(&mut self, symbol: &Symbol) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            let binding = scope.bindings.get(symbol);
            if binding.is_some() {
                return binding;
            }
        }
        None
    }

    fn is_assignable_to(&self, from: TypeId, to: TypeId) -> bool {
        if from == to || to == self.builtin_types.t_any {
            return true;
        }

        false
    }

    fn emit_type_mismatch_error(&mut self, span: Span, expect: TypeId, found: TypeId) {
        self.errors.push(SemanticError::TypeMismatch {
            span,
            expect: self.types.lookup(expect).unwrap().clone(),
            found: self.types.lookup(found).unwrap().clone(),
        });
    }

    fn emit_ir(&mut self, kind: ir::ExprKind, ty: TypeId) -> ir::ExprId {
        let id = ir::ExprId(self.irs.len());
        self.irs.push(Ir { id, kind, ty });
        id
    }

    pub fn analyze(&mut self, program: &[Expression]) -> Vec<ir::ExprId> {
        let mut root_irs = vec![];
        for expr in program {
            let ar = self.handle_expr(expr);
            if let Some(ir) = ar.ir_id {
                root_irs.push(ir);
            }
        }
        root_irs
    }

    pub fn handle_expr(&mut self, expr: &Expression) -> AnalysisResult {
        match &expr.kind {
            ExprKind::Integer(v) => {
                let ir_id = self.emit_ir(ir::ExprKind::Int(*v), self.builtin_types.t_int);
                AnalysisResult {
                    expr_type: self.builtin_types.t_int,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::Float(v) => {
                let ir_id = self.emit_ir(ir::ExprKind::Float(*v), self.builtin_types.t_float);
                AnalysisResult {
                    expr_type: self.builtin_types.t_float,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::Logic(v) => {
                let ir_id = self.emit_ir(ir::ExprKind::Logic(*v), self.builtin_types.t_logic);
                AnalysisResult {
                    expr_type: self.builtin_types.t_logic,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::Char(v) => {
                let ir_id = self.emit_ir(ir::ExprKind::Char(*v), self.builtin_types.t_char);
                AnalysisResult {
                    expr_type: self.builtin_types.t_char,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::Char32(v) => {
                let ir_id = self.emit_ir(ir::ExprKind::Char32(*v), self.builtin_types.t_char32);
                AnalysisResult {
                    expr_type: self.builtin_types.t_char32,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::String(v) => {
                let ir_id =
                    self.emit_ir(ir::ExprKind::String(v.clone()), self.builtin_types.t_string);
                AnalysisResult {
                    expr_type: self.builtin_types.t_string,
                    ir_id: Some(ir_id),
                }
            }
            ExprKind::Decl(e) => self.handle_decl_expr(e.target, e.typ.as_ref(), &e.value, false),
            ExprKind::VarDecl(e) => {
                self.handle_decl_expr(e.name.symbol, Some(&e.typ), &e.expr, true)
            }
            ExprKind::Set(e) => self.handle_set_expr(e),
            ExprKind::Id(e) => self.handle_id_expr(expr.span.clone(), e),
            ExprKind::Block(e) => self.handle_block_expr(e),
            ExprKind::CompareChain(e) => self.handle_compare_chain_expr(e),
            ExprKind::Template(e) => self.handle_template_expr(e),
            ExprKind::Tuple(e) => self.handle_tuple_expr(e),
            ExprKind::If(e) => self.handle_if_expr(e),
            ExprKind::Func(e) => self.handle_func_expr(e),
            ExprKind::Call(e) => self.handle_call_expr(expr.span.clone(), e),
            ExprKind::Binary(e) => self.handle_binary_expr(expr.span.clone(), e),
            ExprKind::Type(e) => AnalysisResult {
                expr_type: self.handle_type_expr(e),
                ir_id: None,
            },
            ExprKind::Member(expr) => self.handle_member_expr(expr),
        }
    }

    fn handle_decl_expr(
        &mut self,
        name: Symbol,
        ty: Option<&TypeExpr>,
        value: &Expression,
        mutable: bool,
    ) -> AnalysisResult {
        let ar = self.handle_expr(value);
        let value_type = ar.expr_type;

        let binding_type = if let Some(typ) = ty
            && !matches!(typ.kind, TypeExprKind::Type)
        {
            let decl_type = self.handle_type_expr(typ);
            if !self.is_assignable_to(value_type, decl_type) {
                self.emit_type_mismatch_error(value.span.clone(), decl_type, value_type);
            }
            decl_type
        } else {
            value_type
        };

        self.declare(
            name,
            Binding {
                type_id: binding_type,
                mutable,
            },
        );

        AnalysisResult {
            expr_type: binding_type,
            ir_id: ar.ir_id.map(|value_ir| {
                self.emit_ir(
                    ir::ExprKind::Set(ir::SetExpr {
                        target: name,
                        value: value_ir,
                    }),
                    binding_type,
                )
            }),
        }
    }

    fn handle_set_expr(&mut self, expr: &SetExpr) -> AnalysisResult {
        let ar = self.handle_expr(&expr.expr);
        let value_type = ar.expr_type;
        let mut type_id = value_type;

        let name = match &expr.target.kind {
            LValueKind::Id(id_expr) => {
                if let Some(binding) = self.lookup(&id_expr.symbol).cloned() {
                    type_id = binding.type_id;
                    if binding.type_id != value_type {
                        self.emit_type_mismatch_error(
                            expr.expr.span.clone(),
                            binding.type_id,
                            value_type,
                        );
                    }
                    if !binding.mutable {
                        self.errors.push(SemanticError::Mutability {
                            span: expr.target.span.clone(),
                            symbol: id_expr.symbol,
                        })
                    }
                }
                id_expr.symbol
            }
        };

        AnalysisResult {
            expr_type: type_id,
            ir_id: ar.ir_id.map(|value_ir| {
                self.emit_ir(
                    ir::ExprKind::Set(ir::SetExpr {
                        target: name,
                        value: value_ir,
                    }),
                    type_id,
                )
            }),
        }
    }

    fn handle_id_expr(&mut self, span: Span, expr: &IdExpr) -> AnalysisResult {
        let type_id = if let Some(binding) = self.lookup(&expr.symbol).cloned() {
            binding.type_id
        } else {
            self.errors.push(SemanticError::Reference {
                span,
                symbol: expr.symbol,
            });
            self.builtin_types.t_any
        };

        let ir = self.emit_ir(ir::ExprKind::Id(expr.symbol), type_id);

        AnalysisResult {
            expr_type: type_id,
            ir_id: Some(ir),
        }
    }

    fn handle_block_expr(&mut self, expr: &BlockExpr) -> AnalysisResult {
        let body_ars: Vec<_> = expr
            .body
            .iter()
            .map(|expr| self.handle_expr(expr))
            .collect();

        let type_id = body_ars
            .last()
            .map(|ar| ar.expr_type)
            .unwrap_or(self.builtin_types.t_void);

        let body_irs = body_ars.iter().map(|ar| ar.ir_id).flatten().collect();

        let ir = self.emit_ir(ir::ExprKind::Block(body_irs), type_id);

        AnalysisResult {
            expr_type: type_id,
            ir_id: Some(ir),
        }
    }

    fn handle_compare_chain_expr(&mut self, expr: &CompareChainExpr) -> AnalysisResult {
        // TODO: check if items are comparable
        // Currently just check if they are the same type

        let head_ar = self.handle_expr(&expr.head);
        let head_type = head_ar.expr_type;

        let mut rest_hir_ids = vec![];
        for (_, expr) in &expr.rest {
            let ar = self.handle_expr(expr);
            if let Some(ir) = ar.ir_id {
                rest_hir_ids.push(ir);
            }
            if ar.expr_type != head_type {
                self.emit_type_mismatch_error(expr.span.clone(), head_type, ar.expr_type);
            }
        }

        let ir = if let Some(head_ir) = head_ar.ir_id
            && rest_hir_ids.len() == expr.rest.len()
        {
            let ir = self.emit_ir(
                ir::ExprKind::CompareChain(ir::CompareChainExpr {
                    head: head_ir,
                    rest: rest_hir_ids
                        .iter()
                        .zip(expr.rest.iter())
                        .map(|(&a, b)| (b.0, a))
                        .collect(),
                }),
                head_type,
            );
            Some(ir)
        } else {
            None
        };

        AnalysisResult {
            expr_type: head_type,
            ir_id: ir,
        }
    }

    fn handle_template_expr(&mut self, expr: &TemplateExpression) -> AnalysisResult {
        let element_irs: Option<Vec<_>> = expr
            .elements
            .iter()
            .map(|el| match el {
                TemplateElement::Expr(expr) => self
                    .handle_expr(expr)
                    .ir_id
                    .map(|ir| ir::TemplateElement::Expr(ir)),
                TemplateElement::Raw(const_id) => Some(ir::TemplateElement::String(*const_id)),
            })
            .collect();

        AnalysisResult {
            expr_type: self.builtin_types.t_string,
            ir_id: element_irs
                .map(|irs| self.emit_ir(ir::ExprKind::Template(irs), self.builtin_types.t_string)),
        }
    }

    fn handle_tuple_expr(&mut self, expr: &TupleExpr) -> AnalysisResult {
        let (elem_types, elem_irs): (Vec<_>, Vec<_>) = expr
            .elements
            .iter()
            .map(|el| {
                let ar = self.handle_expr(el);
                (ar.expr_type, ar.ir_id)
            })
            .unzip();

        let type_id = self.types.intern(TypeInfo::Tuple(elem_types));

        AnalysisResult {
            expr_type: type_id,
            ir_id: if elem_irs.iter().any(|e| e.is_none()) {
                None
            } else {
                Some(self.emit_ir(
                    ir::ExprKind::Tuple(elem_irs.into_iter().flatten().collect()),
                    type_id,
                ))
            },
        }
    }

    fn handle_if_expr(&mut self, expr: &IfExpr) -> AnalysisResult {
        self.push_scope();
        let test_ar = self.handle_expr(&expr.test);
        let then_ar = self.handle_expr(&expr.consequent);
        self.pop_scope();

        let then_type = then_ar.expr_type;

        let (expr_type, alt_ar) = if let Some(alt) = &expr.alternate {
            self.push_scope();
            let ar = self.handle_expr(alt);
            self.pop_scope();
            let expr_type = if then_type != ar.expr_type {
                self.builtin_types.t_any
            } else {
                then_type
            };
            (expr_type, Some(ar))
        } else {
            (self.types.intern(TypeInfo::Option(then_type)), None)
        };

        AnalysisResult {
            expr_type,
            ir_id: if let (Some(test_ir), Some(then_ir)) = (test_ar.ir_id, then_ar.ir_id) {
                Some(self.emit_ir(
                    ir::ExprKind::If(ir::IfExpr {
                        test: test_ir,
                        then: then_ir,
                        alt: alt_ar.map(|ar| ar.ir_id).flatten(),
                    }),
                    expr_type,
                ))
            } else {
                None
            },
        }
    }

    fn handle_func_expr(&mut self, expr: &FunctionExpr) -> AnalysisResult {
        let return_type = self.handle_type_expr(&expr.return_type);

        self.push_scope();

        let param_names: Vec<_> = expr.params.iter().map(|p| p.name).collect();
        let param_types: Vec<_> = expr
            .params
            .iter()
            .map(|p| self.handle_type_expr(&p.typ))
            .collect();

        for (param_name, param_type) in param_names.iter().zip(param_types.iter()) {
            self.declare(
                *param_name,
                Binding {
                    type_id: *param_type,
                    mutable: true,
                },
            );
        }

        let body_ar = self.handle_expr(&expr.body);

        self.pop_scope();

        if return_type != self.builtin_types.t_void {
            if body_ar.expr_type != return_type {
                self.emit_type_mismatch_error(
                    expr.body.span.clone(),
                    return_type,
                    body_ar.expr_type,
                );
            }
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

        AnalysisResult {
            expr_type: type_id,
            ir_id: body_ar.ir_id.map(|body_ir| {
                self.emit_ir(
                    ir::ExprKind::Func(ir::FunctionExpr {
                        name: expr.name,
                        params: param_names,
                        body: body_ir,
                        return_void: return_type == self.builtin_types.t_void,
                    }),
                    type_id,
                )
            }),
        }
    }

    fn handle_type_cast(
        &mut self,
        span: Span,
        args: &[Expression],
        type_id: TypeId,
    ) -> AnalysisResult {
        let ir = if args.len() == 1 {
            self.handle_expr(&args[0]).ir_id.map(|arg| {
                self.emit_ir(
                    ir::ExprKind::Cast {
                        ty: type_id,
                        value: arg,
                    },
                    type_id,
                )
            })
        } else {
            self.errors.push(SemanticError::ArgsCountMismatch { span });
            None
        };
        AnalysisResult {
            expr_type: type_id,
            ir_id: ir,
        }
    }

    fn handle_call_expr(&mut self, span: Span, expr: &CallExpr) -> AnalysisResult {
        let callee_ar = self.handle_expr(&expr.callee);
        let callee_type = callee_ar.expr_type;

        let mut arg_hir_ids = vec![];
        match self.types.lookup(callee_type).cloned().unwrap() {
            TypeInfo::Function { params, ret } => {
                if params.len() != expr.args.len() {
                    self.errors.push(SemanticError::ArgsCountMismatch {
                        span: expr.callee.span.clone(),
                    })
                }
                for (&param_type, arg) in params.iter().zip(expr.args.iter()) {
                    let arg_ar = self.handle_expr(arg);
                    arg_hir_ids.push(arg_ar.ir_id);
                    if !self.is_assignable_to(arg_ar.expr_type, param_type) {
                        self.emit_type_mismatch_error(
                            arg.span.clone(),
                            param_type,
                            arg_ar.expr_type,
                        );
                    }
                }
                AnalysisResult {
                    expr_type: ret,
                    ir_id: if let Some(callee_ir) = callee_ar.ir_id
                        && arg_hir_ids.iter().all(|ir| ir.is_some())
                    {
                        Some(self.emit_ir(
                            ir::ExprKind::Call(ir::CallExpr {
                                callee: callee_ir,
                                args: arg_hir_ids.into_iter().flatten().collect(),
                            }),
                            ret,
                        ))
                    } else {
                        None
                    },
                }
            }
            TypeInfo::Any => {
                // TODO: handle builtin functions
                for arg in &expr.args {
                    arg_hir_ids.push(self.handle_expr(arg).ir_id);
                }
                AnalysisResult {
                    expr_type: self.builtin_types.t_any,
                    ir_id: callee_ar.ir_id.map(|callee| {
                        self.emit_ir(
                            ir::ExprKind::Call(ir::CallExpr {
                                callee,
                                args: arg_hir_ids.into_iter().flatten().collect(),
                            }),
                            self.builtin_types.t_any,
                        )
                    }),
                }
            }
            TypeInfo::Tuple(elements) => {
                let mut ty = self.builtin_types.t_any;
                let ir = if expr.args.len() == 1 {
                    arg_hir_ids.push(self.handle_expr(&expr.args[0]).ir_id);
                    if let ExprKind::Integer(index) = expr.args[0].kind {
                        ty = elements[index as usize];
                        callee_ar.ir_id.map(|tuple| {
                            self.emit_ir(
                                ir::ExprKind::GetTupleElem {
                                    tuple,
                                    index: index as usize,
                                },
                                ty,
                            )
                        })
                    } else {
                        self.errors.push(SemanticError::UnexpectedExpr {
                            span: expr.args[0].span.clone(),
                            expect: "integer".to_string(),
                            found: format!("{:?}", expr.args[0]),
                        });
                        None
                    }
                } else {
                    self.errors.push(SemanticError::ArgsCountMismatch { span });
                    None
                };
                AnalysisResult {
                    expr_type: ty,
                    ir_id: ir,
                }
            }
            TypeInfo::Type(type_id) => self.handle_type_cast(span, &expr.args, type_id),
            TypeInfo::Int => self.handle_type_cast(span, &expr.args, self.builtin_types.t_int),
            _ => {
                self.errors.push(SemanticError::NotCallable {
                    callee: expr.callee.as_ref().clone(),
                });
                AnalysisResult {
                    expr_type: self.builtin_types.t_any,
                    ir_id: None,
                }
            }
        }
    }

    fn handle_member_expr(&mut self, expr: &MemberExpr) -> AnalysisResult {
        let obj_ar = self.handle_expr(&expr.object);

        if let Some(obj_ir) = obj_ar.ir_id {
            if obj_ar.expr_type == self.builtin_types.t_string {
                if let ExprKind::Id(id_expr) = &expr.property.kind {
                    if id_expr.symbol == self.builtin_symbols.s_Length {
                        return AnalysisResult {
                            expr_type: self.builtin_types.t_int,
                            ir_id: Some(self.emit_ir(
                                ir::ExprKind::GetLength(obj_ir),
                                self.builtin_types.t_int,
                            )),
                        };
                    }
                }
            }
        }

        AnalysisResult {
            expr_type: self.builtin_types.t_any,
            ir_id: None,
        }
    }
}

#[derive(Error, Debug)]
pub enum SemanticError {
    #[error("{span:?}: cannot mutate immutable {symbol:?}")]
    Mutability { span: Span, symbol: Symbol },

    #[error("{span:?} mismatched types")]
    TypeMismatch {
        span: Span,
        expect: TypeInfo,
        found: TypeInfo,
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

    #[error("is not callable")]
    NotCallable { callee: Expression },
}
