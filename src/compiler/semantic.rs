use std::collections::HashMap;

use ordermap::OrderMap;
use ordermap::OrderSet;
use thiserror::Error;

use crate::core::{ConstId, PredefinedSymbols, Symbol, SymbolRegistry, types::TypeInfo};

use super::ast::*;
use super::ir::*;
use super::lexer::Span;

pub struct SemanticError {
    pub span: Span,
    pub kind: SemanticErrorKind,
}

#[derive(Debug, Error)]
pub enum SemanticErrorKind {
    #[error("cannot find `{name}` in this scope")]
    UndefinedName { name: String },

    #[error("expected `{expected}`, found `{found}`")]
    TypeMismatch { expected: TypeInfo, found: TypeInfo },

    #[error("cannot apply unary operator `{op}` to type `{operand}`")]
    InvalidUnaryOp { op: UnaryOp, operand: TypeInfo },

    #[error("cannot {op} `{rhs}` to `{lhs}`")]
    InvalidBinaryOp {
        op: BinaryOp,
        lhs: TypeInfo,
        rhs: TypeInfo,
    },

    #[error("cannot assign twice to immutable variable `{name}`")]
    ImmutableAssignment { name: String },

    #[error("invalid left-hand side of assignment")]
    InvalidAssignmentTarget,

    #[error("expected function, found `{ty}`")]
    NotCallable { ty: TypeInfo },

    #[error("expected {expected} argument(s), found {found}")]
    ArgCountMismatch { expected: usize, found: usize },

    #[error("tuple index must be a non-negative integer literal")]
    InvalidTupleIndex,

    #[error("tuple index {index} out of bounds for tuple of length {length}")]
    TupleIndexOutOfBounds { index: i64, length: usize },

    #[error("expected a type, found a value")]
    ExpectedTypeGotValue,

    #[error("`break` is not allowed outside of a loop")]
    BreakOutsideLoop,

    #[error("invalid function effect")]
    InvalidEffect,

    #[error("fallible expression is not allowed in this context")]
    UnexpectedFallibleExpr,

    #[error("expected a fallible expression")]
    ExpectedFallibleExpr,

    #[error("invalid expression")]
    InvalidExpression,

    #[error("need type annotation")]
    TypeAnnotationRequired,

    #[error("struct{} has no field `{field_name}`", .struct_name.as_deref().map(|name| format!(" `{name}`")).unwrap_or_default())]
    UndefinedStructField {
        struct_name: Option<String>,
        field_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub slot: Slot,
    pub type_info: TypeInfo,
    pub mutable: bool,
}

struct LookupResult {
    is_global: bool,
    is_upvalue: bool,
    slot: Slot,
    type_info: TypeInfo,
    mutable: bool,
    scope_index: usize,
}

pub struct Scope {
    is_function: bool,
    next_slot: usize,
    variables: HashMap<Symbol, Variable>,
    upvalues: OrderSet<UpvalueDesc>,
}

impl Scope {
    fn new(is_function: bool) -> Self {
        Self {
            is_function,
            next_slot: 0,
            variables: HashMap::new(),
            upvalues: OrderSet::new(),
        }
    }

    fn declare(&mut self, symbol: Symbol, type_info: TypeInfo, mutable: bool) -> Slot {
        if let Some(binding) = self.variables.get_mut(&symbol) {
            binding.type_info = type_info;
            binding.mutable = mutable;
            return binding.slot;
        }
        let slot = Slot(self.next_slot);
        self.next_slot += 1;
        self.variables.insert(
            symbol,
            Variable {
                slot,
                type_info,
                mutable,
            },
        );
        slot
    }

    fn lookup(&self, symbol: Symbol) -> Option<&Variable> {
        self.variables.get(&symbol)
    }
}

struct LoopInfo {}

#[derive(Clone)]
struct StructInfo {
    name: Option<Symbol>,
    fields: OrderMap<Symbol, (TypeInfo, Ir)>,
}

pub struct SemanticAnalyzer<'a> {
    pub errors: Vec<SemanticError>,

    builtin_symbols: PredefinedSymbols,
    scopes: Vec<Scope>,
    symbol_table: &'a SymbolRegistry,
    loop_stack: Vec<LoopInfo>,
    failure_contexts: u32,
    structs: Vec<StructInfo>,
}

impl<'a> SemanticAnalyzer<'a> {
    pub fn new(symbol_table: &'a mut SymbolRegistry, builtin_symbols: PredefinedSymbols) -> Self {
        let mut global_scope = Scope::new(false);

        let predefined_types = [
            (builtin_symbols.s_int, TypeInfo::Int),
            (builtin_symbols.s_float, TypeInfo::Float),
            (builtin_symbols.s_logic, TypeInfo::Logic),
            (builtin_symbols.s_char, TypeInfo::Char),
            (builtin_symbols.s_char32, TypeInfo::Char32),
            (builtin_symbols.s_string, TypeInfo::String),
            (builtin_symbols.s_any, TypeInfo::Any),
            (builtin_symbols.s_void, TypeInfo::Void),
        ];

        let global_vars = [(
            builtin_symbols.s_Print,
            TypeInfo::Function {
                params: vec![TypeInfo::Any],
                ret: TypeInfo::Void.into(),
            },
        )];

        for (s, t) in predefined_types {
            global_scope.declare(s, TypeInfo::Type(t.into()), false);
        }

        for (s, t) in global_vars {
            global_scope.declare(s, t, false);
        }

        Self {
            scopes: vec![global_scope, Scope::new(true)],
            builtin_symbols,
            errors: vec![],
            symbol_table,
            loop_stack: vec![],
            failure_contexts: 0,
            structs: vec![],
        }
    }

    pub fn get_global_symbol_slots(&self) -> HashMap<Symbol, usize> {
        self.scopes[0]
            .variables
            .iter()
            .map(|(s, v)| (*s, v.slot.0))
            .collect()
    }

    fn push_scope(&mut self, is_function: bool) {
        self.scopes.push(Scope::new(is_function));
    }

    fn pop_scope(&mut self) -> Scope {
        if self.scopes.len() == 1 {
            panic!("cannot pop global scope!");
        }
        self.scopes.pop().unwrap()
    }

    pub fn declare(&mut self, symbol: Symbol, type_info: TypeInfo, mutable: bool) -> Slot {
        // TODO: check shadowing
        self.scopes
            .last_mut()
            .unwrap()
            .declare(symbol, type_info, mutable)
    }

    fn lookup(&mut self, symbol: &Symbol) -> Option<LookupResult> {
        let mut captured = false;
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(var) = scope.lookup(*symbol) {
                let res = LookupResult {
                    is_global: index == 0,
                    is_upvalue: captured,
                    slot: var.slot,
                    type_info: var.type_info.clone(),
                    mutable: var.mutable,
                    scope_index: index,
                };
                return Some(res);
            }
            if scope.is_function {
                captured = true
            }
        }
        None
    }

    fn capture(&mut self, scope_index: usize, slot: Slot) -> usize {
        let mut parent_index = None;
        for scope in self.scopes.iter_mut().skip(scope_index + 1) {
            let desc = if let Some(index) = parent_index {
                UpvalueDesc::Upvalue(index)
            } else {
                UpvalueDesc::Local(slot)
            };
            parent_index = Some(scope.upvalues.insert_full(desc).0);
        }
        parent_index.unwrap()
    }

    fn push_loop(&mut self) {
        self.loop_stack.push(LoopInfo {});
    }

    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    fn push_failure_context(&mut self) {
        self.failure_contexts += 1;
    }

    fn pop_failure_context(&mut self) {
        self.failure_contexts -= 1;
    }

    fn ensure_not_fallible(&mut self, span: &Span) {
        if self.failure_contexts == 0 {
            self.errors.push(SemanticError {
                span: span.clone(),
                kind: SemanticErrorKind::UnexpectedFallibleExpr,
            })
        }
    }

    pub fn analyze(&mut self, program: &[Expression]) -> Vec<Ir> {
        let mut root_irs = vec![];
        for expr in program {
            if let Some(ir) = self.lower_expr(expr) {
                root_irs.push(ir);
            }
        }
        root_irs
    }

    fn lower_expr(&mut self, expr: &Expression) -> Option<Ir> {
        let span = expr.span.clone();
        match &expr.kind {
            ExprKind::Integer(v) => self.lower_int(span, *v).into(),
            ExprKind::Float(v) => self.lower_float(span, *v).into(),
            ExprKind::Logic(v) => self.lower_logic_expr(span, *v).into(),
            ExprKind::Char(v) => self.lower_char(span, *v).into(),
            ExprKind::Char32(v) => self.lower_char32(span, *v).into(),
            ExprKind::String(v) => self.lower_string(span, *v).into(),
            ExprKind::Decl(e) => {
                self.lower_decl_expr(span, &e.target, Some(&e.typ), &e.value, e.mutable)
            }
            ExprKind::Init(e) => self.lower_decl_expr(span, &e.name, None, &e.value, false),
            ExprKind::Set(e) => self.lower_set_expr(span, e),
            ExprKind::Id(e) => self.lower_id_expr(span, e),
            ExprKind::Block(e) => self.lower_block_expr(span, e),
            ExprKind::CompareChain(e) => self.lower_compare_chain_expr(span, e),
            ExprKind::Template(e) => self.lower_template_expr(span, e),
            ExprKind::Tuple(e) => self.lower_tuple_expr(span, e),
            ExprKind::If(e) => self.lower_if_expr(span, e),
            ExprKind::Loop(body) => self.lower_loop_expr(span, body),
            ExprKind::Break => self.lower_break_expr(span),
            ExprKind::Func(e) => self.lower_func_expr(span, e),
            ExprKind::Call(e) => self.lower_call_expr(span, e),
            ExprKind::Binary(e) => self.lower_binary_expr(span, e),
            ExprKind::Unary(e) => self.lower_unary_expr(span, e),
            ExprKind::Type(e) => self.lower_type(span, e).into(),
            ExprKind::Member(expr) => self.lower_member_expr(span, expr),
            ExprKind::Construct(cons_expr) => self.lower_construct_expr(span, cons_expr),
            ExprKind::Query(e) => self.lower_query_expr(span, &e.expr),
        }
    }

    fn try_to_make_type_match(&self, src: &mut TypeInfo, dst: &mut TypeInfo) -> bool {
        if dst == &TypeInfo::Any {
            return true;
        }

        if let TypeInfo::Unknown { infer } = dst {
            if *infer {
                *dst = src.clone();
            }
            return true;
        }

        match (src, dst) {
            (TypeInfo::Option(a), TypeInfo::Option(b)) => {
                self.try_to_make_type_match(a.as_mut(), b)
            }
            (src, dst) => src == dst,
        }
    }

    fn ensure_type_match(&mut self, mut expected: &mut TypeInfo, found: &mut Ir) {
        // try to unwrap container types to make the error range small
        match (&mut expected, &mut found.kind) {
            (TypeInfo::Tuple(e), IrKind::Tuple(irs)) if e.len() == irs.len() => {
                for (e, ir) in e.iter_mut().zip(irs.iter_mut()) {
                    self.ensure_type_match(e, ir);
                }
                return;
            }
            (
                TypeInfo::Option(e),
                IrKind::Option(Some(ir))
                | IrKind::If(IfIr {
                    then: ir,
                    alt: None,
                    ..
                }),
            ) => return self.ensure_type_match(e, ir),
            (
                e,
                IrKind::If(IfIr {
                    then,
                    alt: Some(alt),
                    ..
                }),
            ) => {
                self.ensure_type_match(e, then);
                self.ensure_type_match(e, alt);
                return;
            }
            (e, IrKind::Block(irs)) => match irs.as_mut_slice() {
                [.., ir] => return self.ensure_type_match(e, ir),
                _ => {}
            },
            (e @ TypeInfo::Option(_), IrKind::Logic(false)) => {
                found.ty = e.clone();
                found.kind = IrKind::Option(None);
                return;
            }
            _ => {}
        }

        if !self.try_to_make_type_match(&mut found.ty, expected) {
            self.errors.push(SemanticError {
                span: found.span.clone(),
                kind: SemanticErrorKind::TypeMismatch {
                    expected: expected.clone(),
                    found: found.ty.clone(),
                },
            });
        }
    }

    fn lower_int(&mut self, span: Span, value: i64) -> Ir {
        Ir {
            span,
            ty: TypeInfo::Int,
            kind: IrKind::Int(value),
        }
    }

    fn lower_float(&mut self, span: Span, value: f64) -> Ir {
        Ir {
            span,
            ty: TypeInfo::Float,
            kind: IrKind::Float(value),
        }
    }

    fn lower_logic_expr(&mut self, span: Span, value: bool) -> Ir {
        Ir {
            span,
            ty: TypeInfo::Logic,
            kind: IrKind::Logic(value),
        }
    }

    fn lower_char(&self, span: Span, value: u8) -> Ir {
        Ir {
            span,
            ty: TypeInfo::Char,
            kind: IrKind::Char(value),
        }
    }

    fn lower_char32(&self, span: Span, value: char) -> Ir {
        Ir {
            span,
            ty: TypeInfo::Char32,
            kind: IrKind::Char32(value),
        }
    }

    fn lower_string(&self, span: Span, const_id: ConstId) -> Ir {
        Ir {
            span,
            kind: IrKind::String(const_id),
            ty: TypeInfo::String,
        }
    }

    fn lower_decl_expr(
        &mut self,
        span: Span,
        name: &IdExpr,
        ty: Option<&TypeExpr>,
        value: &Expression,
        mutable: bool,
    ) -> Option<Ir> {
        let mut binding_type = ty.map(|ty| self.parse_type_expr(ty));
        let mut value_ir = self.lower_expr(value)?;

        if let Some(binding_type) = &mut binding_type {
            self.ensure_type_match(binding_type, &mut value_ir);
        }

        let binding_type = binding_type.unwrap_or_else(|| value_ir.ty.clone());
        if !binding_type.is_complete() {
            self.errors.push(SemanticError {
                span: name.span.clone(),
                kind: SemanticErrorKind::TypeAnnotationRequired,
            });
        }

        let slot = self.declare(name.symbol, binding_type.clone(), mutable);

        if let TypeInfo::Type(inner_type) = &binding_type {
            match **inner_type {
                TypeInfo::Struct { id } => {
                    // Give struct a name ;)
                    self.structs[id as usize].name = Some(name.symbol);
                }
                _ => {}
            }
        }

        Some(Ir {
            span,
            ty: binding_type,
            kind: IrKind::StoreLocal {
                slot,
                value: Box::new(value_ir),
            },
        })
    }

    fn lower_set_expr(&mut self, span: Span, expr: &SetExpr) -> Option<Ir> {
        match &expr.lhs.kind {
            ExprKind::Id(id_expr) => {
                let mut var = match self.lookup(&id_expr.symbol) {
                    Some(var) => var,
                    None => {
                        self.errors.push(SemanticError {
                            span: expr.lhs.span.clone(),
                            kind: SemanticErrorKind::UndefinedName {
                                name: self.symbol_table.lookup(id_expr.symbol).to_string(),
                            },
                        });
                        return None;
                    }
                };

                if !var.mutable {
                    self.errors.push(SemanticError {
                        span: expr.lhs.span.clone(),
                        kind: SemanticErrorKind::ImmutableAssignment {
                            name: self.symbol_table.lookup(id_expr.symbol).to_string(),
                        },
                    })
                }

                let mut value = self.lower_expr(&expr.rhs)?;
                self.ensure_type_match(&mut var.type_info, &mut value);
                return Some(self.make_store_ir(span, &var, value));
            }
            ExprKind::Member(member_expr) => {
                let obj = self.lower_expr(&member_expr.object)?;
                match &obj.ty {
                    TypeInfo::Struct { id } => {
                        let mut value_ir = self.lower_expr(&expr.rhs)?;
                        let struct_info = &self.structs[*id as usize];
                        let field = struct_info.fields.get_full(&member_expr.property.symbol);
                        return if let Some((index, _, (ty, _))) = field {
                            let mut expected = ty.clone();
                            self.ensure_type_match(&mut expected, &mut value_ir);
                            Some(Ir {
                                span,
                                ty: expected,
                                kind: IrKind::SetStructField {
                                    obj: obj.into(),
                                    index,
                                    value: value_ir.into(),
                                },
                            })
                        } else {
                            self.errors.push(SemanticError {
                                span,
                                kind: SemanticErrorKind::UndefinedStructField {
                                    struct_name: struct_info
                                        .name
                                        .map(|symbol| self.symbol_table.lookup(symbol).to_string()),
                                    field_name: self
                                        .symbol_table
                                        .lookup(member_expr.property.symbol)
                                        .to_string(),
                                },
                            });
                            None
                        };
                    }
                    _ => {}
                }
            }
            ExprKind::Call(_) => {
                let callee = self.lower_expr(&expr.lhs)?;
                match callee.kind {
                    IrKind::IndexTuple { tuple, index } => {
                        let value = self.lower_expr(&expr.rhs)?;
                        return Some(Ir {
                            span,
                            ty: callee.ty,
                            kind: IrKind::SetTupleElement {
                                obj: tuple,
                                index,
                                value: value.into(),
                            },
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        self.errors.push(SemanticError {
            span: expr.lhs.span.clone(),
            kind: SemanticErrorKind::InvalidAssignmentTarget,
        });
        None
    }

    fn lower_id_expr(&mut self, span: Span, expr: &IdExpr) -> Option<Ir> {
        if let Some(var) = self.lookup(&expr.symbol) {
            Some(self.make_load_ir(span, &var))
        } else {
            self.errors.push(SemanticError {
                span,
                kind: SemanticErrorKind::UndefinedName {
                    name: self.symbol_table.lookup(expr.symbol).to_string(),
                },
            });
            None
        }
    }

    fn make_load_ir(&mut self, span: Span, var: &LookupResult) -> Ir {
        let kind = if var.is_global {
            IrKind::LoadGlobal { slot: var.slot }
        } else if var.is_upvalue {
            let index = self.capture(var.scope_index, var.slot);
            IrKind::LoadUpvalue { index }
        } else {
            IrKind::LoadLocal { slot: var.slot }
        };
        Ir {
            span,
            ty: var.type_info.clone(),
            kind,
        }
    }

    fn make_store_ir(&mut self, span: Span, var: &LookupResult, value: Ir) -> Ir {
        let kind = if var.is_global {
            IrKind::StoreGlobal {
                slot: var.slot,
                value: Box::new(value),
            }
        } else if var.is_upvalue {
            IrKind::StoreUpvalue {
                index: self.capture(var.scope_index, var.slot),
                value: Box::new(value),
            }
        } else {
            IrKind::StoreLocal {
                slot: var.slot,
                value: Box::new(value),
            }
        };
        Ir {
            span,
            ty: var.type_info.clone(),
            kind,
        }
    }

    fn lower_block_expr(&mut self, span: Span, expr: &BlockExpr) -> Option<Ir> {
        let mut body = Vec::with_capacity(expr.body.len());
        for e in expr.body.iter() {
            body.push(self.lower_expr(e));
        }

        if body.last().is_some_and(|ir| ir.is_none()) {
            return None;
        }

        let body: Vec<_> = body.into_iter().flatten().collect();

        Some(Ir {
            span,
            ty: body
                .last()
                .map(|ir| ir.ty.clone())
                .unwrap_or(TypeInfo::Void),
            kind: IrKind::Block(body),
        })
    }

    fn lower_compare_chain_expr(&mut self, span: Span, expr: &CompareChainExpr) -> Option<Ir> {
        // TODO: check if items are comparable
        // Currently just check if they are the same type

        self.ensure_not_fallible(&span);

        let head_ir = self.lower_expr(&expr.head)?;
        let mut rest_irs = Vec::with_capacity(expr.rest.len());

        for (_, expr) in &expr.rest {
            let ir = self.lower_expr(expr)?;
            if ir.ty != head_ir.ty {
                self.errors.push(SemanticError {
                    span: expr.span.clone(),
                    kind: SemanticErrorKind::TypeMismatch {
                        expected: head_ir.ty.clone(),
                        found: ir.ty.clone(),
                    },
                });
            }
            rest_irs.push(ir);
        }

        Some(Ir {
            span,
            ty: head_ir.ty.clone(),
            kind: IrKind::CompareChain(CompareChainIr {
                head: head_ir.into(),
                rest: rest_irs
                    .into_iter()
                    .zip(expr.rest.iter())
                    .map(|(ir, (op, _))| (*op, ir))
                    .collect(),
            }),
        })
    }

    fn lower_template_expr(&mut self, span: Span, expr: &TemplateExpression) -> Option<Ir> {
        let mut elements = Vec::with_capacity(expr.elements.len());
        for el in expr.elements.iter() {
            match el {
                TemplateElement::Expr(expr) => {
                    elements.push(TemplateElementIr::Expr(self.lower_expr(expr)?.into()));
                }
                TemplateElement::Raw(const_id) => {
                    elements.push(TemplateElementIr::String(*const_id));
                }
            }
        }

        Some(Ir {
            span,
            kind: IrKind::Template(elements),
            ty: TypeInfo::String,
        })
    }

    fn lower_tuple_expr(&mut self, span: Span, expr: &TupleExpr) -> Option<Ir> {
        let mut types = Vec::with_capacity(expr.elements.len());
        let mut irs = Vec::with_capacity(expr.elements.len());

        for el in expr.elements.iter() {
            let ir = self.lower_expr(el)?;
            types.push(ir.ty.clone());
            irs.push(ir);
        }

        Some(Ir {
            span,
            ty: TypeInfo::Tuple(types),
            kind: IrKind::Tuple(irs),
        })
    }

    fn lower_if_expr(&mut self, span: Span, expr: &IfExpr) -> Option<Ir> {
        self.push_scope(false);
        self.push_failure_context();
        let test_ir = self.lower_expr(&expr.test);
        self.pop_failure_context();
        let then_ir = self.lower_expr(&expr.consequent);
        self.pop_scope();

        let test_ir = test_ir?;
        if !test_ir.kind.is_fallible() {
            self.errors.push(SemanticError {
                span: expr.test.span.clone(),
                kind: SemanticErrorKind::ExpectedFallibleExpr,
            })
        }

        let then_ir = then_ir?;

        let (expr_type, alt_ir) = if let Some(alt) = &expr.alternate {
            self.push_scope(false);
            let alt_ir = self.lower_expr(alt);
            self.pop_scope();

            let alt_ir = alt_ir?;
            let expr_type = if then_ir.ty != alt_ir.ty {
                TypeInfo::Any
            } else {
                then_ir.ty.clone()
            };
            (expr_type, Some(alt_ir))
        } else {
            (TypeInfo::Option(then_ir.ty.clone().into()), None)
        };

        Some(Ir {
            span,
            ty: expr_type,
            kind: IrKind::If(IfIr {
                test: test_ir.into(),
                then: then_ir.into(),
                alt: alt_ir.map(|ir| ir.into()),
            }),
        })
    }

    fn lower_loop_expr(&mut self, span: Span, body: &Expression) -> Option<Ir> {
        self.push_loop();
        let body_ir = self.lower_expr(body);
        self.pop_loop();
        Some(Ir {
            span,
            ty: TypeInfo::True,
            kind: IrKind::Loop(body_ir?.into()),
        })
    }

    fn lower_break_expr(&mut self, span: Span) -> Option<Ir> {
        if self.loop_stack.is_empty() {
            self.errors.push(SemanticError {
                span: span,
                kind: SemanticErrorKind::BreakOutsideLoop,
            });
            return None;
        }

        Some(Ir {
            span,
            ty: TypeInfo::Bottom,
            kind: IrKind::Break,
        })
    }

    fn lower_func_expr(&mut self, span: Span, expr: &FunctionExpr) -> Option<Ir> {
        let mut return_type = self.parse_type_expr(&expr.return_type);
        let param_names: Vec<_> = expr.params.iter().map(|p| p.name).collect();
        let param_types: Vec<_> = expr
            .params
            .iter()
            .map(|p| self.parse_type_expr(&p.typ))
            .collect();

        let mut effects = Effects { decides: false };
        for effect in expr.effects.iter() {
            match effect.symbol {
                e if e == self.builtin_symbols.s_decides => {
                    effects.decides = true;
                }
                _ => self.errors.push(SemanticError {
                    span: effect.span.clone(),
                    kind: SemanticErrorKind::InvalidEffect,
                }),
            }
        }

        self.push_scope(true);

        let mut param_slots = vec![];
        for (param_name, param_type) in param_names.into_iter().zip(param_types.iter()) {
            let slot = self.declare(param_name, param_type.clone(), true);
            param_slots.push(slot);
        }

        let body = self.lower_expr(&expr.body);

        let scope = self.pop_scope();

        let mut body = body?;

        if return_type != TypeInfo::Void {
            self.ensure_type_match(&mut return_type, &mut body);
        }

        let return_void = return_type == TypeInfo::Void;
        let type_info = TypeInfo::Function {
            params: param_types,
            ret: return_type.into(),
        };

        let upvalues = scope.upvalues.into_iter().collect();
        let func_slot = self.declare(expr.name, type_info.clone(), false);

        Some(Ir {
            span,
            ty: type_info,
            kind: IrKind::Func(FunctionIr {
                slot: func_slot,
                params: param_slots,
                effects,
                body: body.into(),
                return_void,
                upvalues,
            }),
        })
    }

    fn lower_type_cast(
        &mut self,
        span: Span,
        args: &[Expression],
        type_info: &TypeInfo,
    ) -> Option<Ir> {
        self.ensure_not_fallible(&span);

        if args.len() != 1 {
            self.errors.push(SemanticError {
                span: span.clone(),
                kind: SemanticErrorKind::ArgCountMismatch {
                    expected: 1,
                    found: args.len(),
                },
            });
        }

        if let Some(arg) = args.first() {
            let arg = self.lower_expr(arg)?;
            Some(Ir {
                span,
                kind: IrKind::Cast {
                    ty: type_info.clone(),
                    value: arg.into(),
                },
                ty: type_info.clone(),
            })
        } else {
            None
        }
    }

    fn lower_call_expr(&mut self, span: Span, expr: &CallExpr) -> Option<Ir> {
        let mut callee_ir = self.lower_expr(&expr.callee)?;

        match &mut callee_ir.ty {
            TypeInfo::Function { params, ret } => {
                if params.len() != expr.args.len() {
                    self.errors.push(SemanticError {
                        span: expr.callee.span.clone(),
                        kind: SemanticErrorKind::ArgCountMismatch {
                            expected: params.len(),
                            found: expr.args.len(),
                        },
                    })
                }

                let mut arg_irs = Vec::with_capacity(params.len());
                for (param_type, arg) in params.iter_mut().zip(expr.args.iter()) {
                    let mut arg_ir = self.lower_expr(arg)?;
                    self.ensure_type_match(param_type, &mut arg_ir);
                    arg_irs.push(arg_ir);
                }

                Some(Ir {
                    span,
                    ty: (**ret).clone(),
                    kind: IrKind::Call(CallIr {
                        callee: callee_ir.into(),
                        args: arg_irs,
                    }),
                })
            }
            TypeInfo::Tuple(_) => {
                if expr.fallible || expr.args.len() != 1 {
                    self.errors.push(SemanticError {
                        span: span.clone(),
                        kind: SemanticErrorKind::InvalidExpression,
                    });
                    return None;
                }
                self.lower_tuple_index(span, callee_ir, &expr.args[0])
            }
            TypeInfo::Array(_) => {
                self.ensure_not_fallible(&span);
                if !expr.fallible || expr.args.len() != 1 {
                    self.errors.push(SemanticError {
                        span: span.clone(),
                        kind: SemanticErrorKind::InvalidExpression,
                    });
                    return None;
                }
                self.lower_array_index(span, callee_ir, &expr.args[0])
            }
            TypeInfo::Type(type_id) => {
                if expr.fallible || expr.args.len() != 1 {
                    self.errors.push(SemanticError {
                        span: span.clone(),
                        kind: SemanticErrorKind::InvalidExpression,
                    });
                    return None;
                }
                self.lower_type_cast(span, &expr.args, type_id)
            }
            _ => {
                self.errors.push(SemanticError {
                    span: expr.callee.span.clone(),
                    kind: SemanticErrorKind::NotCallable {
                        ty: callee_ir.ty.clone(),
                    },
                });
                None
            }
        }
    }

    fn lower_tuple_index(&mut self, span: Span, tuple_ir: Ir, arg_expr: &Expression) -> Option<Ir> {
        let arg_ir = self.lower_expr(arg_expr)?;

        let elements = match &tuple_ir.ty {
            TypeInfo::Tuple(elements) => elements,
            _ => unreachable!(),
        };

        match arg_ir.kind {
            IrKind::Int(index) if index >= 0 && (index as usize) < elements.len() => Some(Ir {
                span,
                ty: elements[index as usize].clone(),
                kind: IrKind::IndexTuple {
                    tuple: Box::new(tuple_ir),
                    index: index as usize,
                },
            }),
            IrKind::Int(index) => {
                self.errors.push(SemanticError {
                    span: arg_expr.span.clone(),
                    kind: SemanticErrorKind::TupleIndexOutOfBounds {
                        index,
                        length: elements.len(),
                    },
                });
                None
            }
            _ => {
                self.errors.push(SemanticError {
                    span: arg_expr.span.clone(),
                    kind: SemanticErrorKind::InvalidTupleIndex,
                });
                None
            }
        }
    }

    fn lower_array_index(&mut self, span: Span, array_ir: Ir, index: &Expression) -> Option<Ir> {
        let index_ir = self.lower_expr(index)?;

        let item_type = match &array_ir.ty {
            TypeInfo::Array(item_type) => (**item_type).clone(),
            _ => TypeInfo::Unknown { infer: true },
        };

        Some(Ir {
            span,
            ty: item_type,
            kind: IrKind::IndexArray {
                array: array_ir.into(),
                index: index_ir.into(),
            },
        })
    }

    fn lower_binary_expr(&mut self, span: Span, expr: &BinaryExpr) -> Option<Ir> {
        let lhs = self.lower_expr(&expr.lhs)?;
        let rhs = self.lower_expr(&expr.rhs)?;

        let ir_type = match (&lhs.ty, &rhs.ty) {
            (TypeInfo::String, TypeInfo::String) => {
                return self.lower_string_binary(span, expr, lhs, rhs);
            }
            (TypeInfo::Int, TypeInfo::Int) => TypeInfo::Int,
            (TypeInfo::Float, TypeInfo::Float) => TypeInfo::Float,
            (TypeInfo::Rational, TypeInfo::Rational) => TypeInfo::Rational,
            _ => {
                self.errors.push(SemanticError {
                    span: expr.op_span.clone(),
                    kind: SemanticErrorKind::InvalidBinaryOp {
                        op: expr.op,
                        lhs: lhs.ty,
                        rhs: rhs.ty,
                    },
                });
                return None;
            }
        };

        let kind = match expr.op {
            BinaryOp::Add => IrKind::Add((lhs.into(), rhs.into())),
            BinaryOp::Sub => IrKind::Sub((lhs.into(), rhs.into())),
            BinaryOp::Mul => IrKind::Mul((lhs.into(), rhs.into())),
            BinaryOp::Div => IrKind::Div((lhs.into(), rhs.into())),
        };

        Some(Ir {
            span,
            ty: ir_type,
            kind,
        })
    }

    fn lower_string_binary(
        &mut self,
        span: Span,
        expr: &BinaryExpr,
        lhs: Ir,
        rhs: Ir,
    ) -> Option<Ir> {
        if expr.op != BinaryOp::Add {
            self.errors.push(SemanticError {
                span: expr.op_span.clone(),
                kind: SemanticErrorKind::InvalidBinaryOp {
                    op: expr.op,
                    lhs: lhs.ty.clone(),
                    rhs: rhs.ty.clone(),
                },
            });
            return None;
        }

        if rhs.ty != lhs.ty {
            self.errors.push(SemanticError {
                span,
                kind: SemanticErrorKind::TypeMismatch {
                    expected: lhs.ty,
                    found: rhs.ty,
                },
            });
            return None;
        }

        let mut irs = vec![];
        flatten_add_ir(lhs, &mut irs);
        flatten_add_ir(rhs, &mut irs);
        Some(Ir {
            span,
            ty: TypeInfo::String,
            kind: IrKind::Concat(irs),
        })
    }

    fn lower_unary_expr(&mut self, span: Span, expr: &UnaryExpr) -> Option<Ir> {
        let ir = self.lower_expr(&expr.expr)?;
        match expr.op {
            UnaryOp::Plus | UnaryOp::Minus => {
                let expected_types = vec![TypeInfo::Int, TypeInfo::Float, TypeInfo::Rational];
                if expected_types.contains(&ir.ty) {
                    let ty = ir.ty.clone();
                    let kind = match expr.op {
                        UnaryOp::Minus => IrKind::Neg(ir.into()),
                        _ => ir.kind,
                    };
                    Some(Ir { span, ty, kind })
                } else {
                    self.errors.push(SemanticError {
                        span: expr.expr.span.clone(),
                        kind: SemanticErrorKind::InvalidUnaryOp {
                            op: expr.op,
                            operand: ir.ty,
                        },
                    });
                    None
                }
            }
            UnaryOp::Not => {
                if ir.ty == TypeInfo::Logic {
                    Some(Ir {
                        span,
                        ty: TypeInfo::Logic,
                        kind: IrKind::Not(ir.into()),
                    })
                } else {
                    self.errors.push(SemanticError {
                        span: expr.expr.span.clone(),
                        kind: SemanticErrorKind::InvalidUnaryOp {
                            op: expr.op,
                            operand: ir.ty,
                        },
                    });
                    None
                }
            }
        }
    }

    fn parse_type_expr(&mut self, expr: &TypeExpr) -> TypeInfo {
        match &expr.kind {
            TypeExprKind::Named(symbol) => {
                if let Some(binding) = self.lookup(symbol) {
                    if let TypeInfo::Type(inner_type) = binding.type_info {
                        return *inner_type;
                    }
                    self.errors.push(SemanticError {
                        span: expr.span.clone(),
                        kind: SemanticErrorKind::ExpectedTypeGotValue,
                    });
                } else {
                    self.errors.push(SemanticError {
                        span: expr.span.clone(),
                        kind: SemanticErrorKind::UndefinedName {
                            name: self.symbol_table.lookup(*symbol).to_string(),
                        },
                    });
                }
                TypeInfo::Any
            }
            TypeExprKind::Option(inner) => TypeInfo::Option(self.parse_type_expr(inner).into()),
            TypeExprKind::Tuple(args) => {
                TypeInfo::Tuple(args.iter().map(|arg| self.parse_type_expr(arg)).collect())
            }
            TypeExprKind::Array(elem_type) => {
                TypeInfo::Array(self.parse_type_expr(elem_type).into())
            }
            TypeExprKind::Struct(fields) => {
                let mut field_infos = OrderMap::new();
                for f in fields {
                    let name = f.name.symbol;
                    let mut ty = self.parse_type_expr(&f.ty);
                    match self.lower_expr(&f.default) {
                        Some(mut ir) => {
                            self.ensure_type_match(&mut ty, &mut ir);
                            field_infos.insert(name, (ty, ir));
                        }
                        _ => {
                            field_infos.insert(
                                name,
                                (
                                    ty.clone(),
                                    Ir {
                                        span: f.default.span.clone(),
                                        ty,
                                        kind: IrKind::Break, // TODO: replace it with a placeholder IrKind
                                    },
                                ),
                            );
                        }
                    };
                }
                let struct_id = self.structs.len() as u32;
                self.structs.push(StructInfo {
                    name: None,
                    fields: field_infos,
                });
                TypeInfo::Struct { id: struct_id }
            }
            TypeExprKind::Function { params, ret } => TypeInfo::Function {
                params: params.iter().map(|p| self.parse_type_expr(p)).collect(),
                ret: self.parse_type_expr(ret).into(),
            },
            TypeExprKind::Type => TypeInfo::Type(TypeInfo::Unknown { infer: true }.into()),
        }
    }

    fn lower_type(&mut self, span: Span, expr: &TypeExpr) -> Ir {
        let type_info = self.parse_type_expr(expr);
        Ir {
            span,
            ty: TypeInfo::Type(type_info.clone().into()),
            kind: IrKind::Type(type_info),
        }
    }

    fn lower_member_expr(&mut self, span: Span, expr: &MemberExpr) -> Option<Ir> {
        let obj = self.lower_expr(&expr.object)?;

        match obj.ty {
            TypeInfo::String | TypeInfo::Array(_) => {
                if expr.property.symbol == self.builtin_symbols.s_Length {
                    return Some(Ir {
                        span,
                        kind: IrKind::GetLength(obj.into()),
                        ty: TypeInfo::Int,
                    });
                }
            }
            TypeInfo::Struct { id } => {
                let struct_info = &self.structs[id as usize];
                let struct_name = struct_info.name;
                return if let Some((field_index, _, (ty, _))) =
                    struct_info.fields.get_full(&expr.property.symbol)
                {
                    Some(Ir {
                        span,
                        ty: ty.clone(),
                        kind: IrKind::GetStructField {
                            obj: obj.into(),
                            index: field_index,
                        },
                    })
                } else {
                    self.errors.push(SemanticError {
                        span: expr.property.span.clone(),
                        kind: SemanticErrorKind::UndefinedStructField {
                            struct_name: struct_name
                                .map(|symbol| self.symbol_table.lookup(symbol).to_string()),
                            field_name: self.symbol_table.lookup(expr.property.symbol).to_string(),
                        },
                    });
                    None
                };
            }
            _ => {}
        }

        todo!()
    }

    fn lower_construct_expr(&mut self, span: Span, cons_expr: &ConstructExpr) -> Option<Ir> {
        if let ExprKind::Id(id_expr) = &cons_expr.callee.kind {
            if id_expr.symbol == self.builtin_symbols.s_option {
                return self.lower_construct_option(span, cons_expr);
            }
            if id_expr.symbol == self.builtin_symbols.s_array {
                return self.lower_construct_array(span, cons_expr);
            }
            if let Some(var) = self.lookup(&id_expr.symbol) {
                match var.type_info {
                    TypeInfo::Type(t) => match *t {
                        TypeInfo::Struct { id } => {
                            let struct_info = &self.structs[id as usize];
                            let struct_name = struct_info.name;
                            let mut init_fields = struct_info.fields.clone();

                            for arg in cons_expr.args.iter() {
                                match &arg.kind {
                                    ExprKind::Init(e) => {
                                        if let Some((index, _, (ty, _))) =
                                            init_fields.get_full_mut(&e.name.symbol)
                                        {
                                            let mut value_ir = self.lower_expr(&e.value)?;
                                            self.ensure_type_match(ty, &mut value_ir);
                                            init_fields[index].1 = value_ir;
                                        } else {
                                            self.errors.push(SemanticError {
                                                span: e.name.span.clone(),
                                                kind: SemanticErrorKind::UndefinedStructField {
                                                    struct_name: struct_name.map(|symbol| {
                                                        self.symbol_table.lookup(symbol).to_string()
                                                    }),
                                                    field_name: self
                                                        .symbol_table
                                                        .lookup(e.name.symbol)
                                                        .to_string(),
                                                },
                                            })
                                        }
                                    }
                                    _ => self.errors.push(SemanticError {
                                        span: arg.span.clone(),
                                        kind: SemanticErrorKind::InvalidExpression,
                                    }),
                                }
                            }

                            return Some(Ir {
                                span,
                                ty: (*t).clone(),
                                kind: IrKind::MakeStruct {
                                    fields: init_fields.into_values().map(|(.., ir)| ir).collect(),
                                },
                            });
                        }
                        _ => {
                            todo!("{:?}", t);
                        }
                    },
                    _ => {}
                }
            }
        }

        todo!("{:?}", cons_expr);
    }

    fn lower_construct_option(&mut self, span: Span, cons_expr: &ConstructExpr) -> Option<Ir> {
        if cons_expr.args.len() == 1 {
            let value_ir = self.lower_expr(&cons_expr.args[0])?;
            let mut value_ty = value_ir.ty.clone();

            if let IrKind::Logic(false) = value_ir.kind {
                // V := option{ false }
                //                ^-- unknown inner type
                value_ty = TypeInfo::Unknown { infer: true };
            }

            Some(Ir {
                span,
                ty: TypeInfo::Option(value_ty.into()),
                kind: IrKind::Option(Some(value_ir.into())),
            })
        } else {
            self.errors.push(SemanticError {
                span,
                kind: SemanticErrorKind::InvalidExpression,
            });
            None
        }
    }

    fn lower_construct_array(&mut self, span: Span, cons_expr: &ConstructExpr) -> Option<Ir> {
        let mut irs: Vec<Ir> = Vec::with_capacity(cons_expr.args.len());
        for arg in cons_expr.args.iter() {
            let ir = self.lower_expr(arg)?;
            irs.push(ir);
        }

        let all_items_has_same_type = irs
            .iter()
            .zip(irs.iter().skip(1))
            .all(|(a, b)| a.ty == b.ty);

        Some(Ir {
            span,
            ty: TypeInfo::Array(
                if all_items_has_same_type {
                    irs.first()
                        .map(|ir| ir.ty.clone())
                        .unwrap_or(TypeInfo::Unknown { infer: true })
                } else {
                    TypeInfo::Any
                }
                .into(),
            ),
            kind: IrKind::Array(irs),
        })
    }

    fn lower_query_expr(&mut self, span: Span, expr: &Expression) -> Option<Ir> {
        self.ensure_not_fallible(&span);

        let ir = self.lower_expr(expr)?;

        match ir.ty.clone() {
            TypeInfo::Option(inner_type) => Some(Ir {
                span,
                ty: *inner_type,
                kind: IrKind::Unwrap(ir.into()),
            }),
            _ => {
                self.errors.push(SemanticError {
                    span,
                    kind: SemanticErrorKind::InvalidExpression,
                });
                None
            }
        }
    }
}

fn flatten_add_ir(ir: Ir, out: &mut Vec<Ir>) {
    if let IrKind::Add((lhs, rhs)) = ir.kind {
        flatten_add_ir(*lhs, out);
        flatten_add_ir(*rhs, out);
    } else {
        out.push(ir);
    }
}
