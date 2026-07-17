use ordermap::OrderSet;
use std::collections::HashMap;

use crate::core::{
    PredefinedSymbols, Symbol, SymbolRegistry,
    types::{TypeInfo, TypeRegistry},
};

use super::ast::*;
use super::ir::*;
use super::lexer::Span;

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

pub struct SemanticAnalyzer {
    pub builtin_symbols: PredefinedSymbols,
    pub errors: Vec<SemanticError>,

    pub scopes: Vec<Scope>,
    pub types: TypeRegistry,

    loop_stack: Vec<LoopInfo>,
    failure_contexts: u32,
}

impl SemanticAnalyzer {
    pub fn new(symbol_table: &mut SymbolRegistry) -> Self {
        let mut global_scope = Scope::new(false);

        let bs = PredefinedSymbols::install(symbol_table);

        let predefined_types = [
            (bs.s_int, TypeInfo::Int),
            (bs.s_float, TypeInfo::Float),
            (bs.s_logic, TypeInfo::Logic),
            (bs.s_char, TypeInfo::Char),
            (bs.s_char32, TypeInfo::Char32),
            (bs.s_string, TypeInfo::String),
            (bs.s_any, TypeInfo::Any),
            (bs.s_void, TypeInfo::Void),
        ];

        let global_vars = [(
            bs.s_Print,
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
            scopes: vec![global_scope],
            builtin_symbols: bs,
            errors: vec![],
            types: TypeRegistry::default(),
            loop_stack: vec![],
            failure_contexts: 0,
        }
    }

    pub fn get_global_symbol_index(&self, symbol: Symbol) -> usize {
        self.scopes[0].lookup(symbol).unwrap().slot.0
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

    fn is_assignable_to(&self, from: &TypeInfo, to: &TypeInfo) -> bool {
        if from == to || to == &TypeInfo::Any {
            return true;
        }

        match (from, to) {
            (TypeInfo::True | TypeInfo::False, TypeInfo::Logic) => true,
            (TypeInfo::False, TypeInfo::Option(_)) => true,
            (TypeInfo::Option(from), TypeInfo::Option(to)) => self.is_assignable_to(from, to),
            _ => false,
        }
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

    fn ensure_not_fallible(&mut self, span: Span) {
        if self.failure_contexts == 0 {
            self.errors
                .push(SemanticError::UnexpectedFallibleExpr { span })
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

    pub fn lower_expr(&mut self, expr: &Expression) -> Option<Ir> {
        match &expr.kind {
            ExprKind::Integer(v) => Some(Ir {
                kind: IrKind::Int(*v),
                ty: TypeInfo::Int,
            }),
            ExprKind::Float(v) => Some(Ir {
                kind: IrKind::Float(*v),
                ty: TypeInfo::Float,
            }),
            ExprKind::Logic(v) => Some(self.lower_logic_expr(*v)),
            ExprKind::Char(v) => Some(Ir {
                kind: IrKind::Char(*v),
                ty: TypeInfo::Char,
            }),
            ExprKind::Char32(v) => Some(Ir {
                kind: IrKind::Char32(*v),
                ty: TypeInfo::Char32,
            }),
            ExprKind::String(v) => Some(Ir {
                kind: IrKind::String(*v),
                ty: TypeInfo::String,
            }),
            ExprKind::Decl(e) => self.lower_decl_expr(e.target, e.typ.as_ref(), &e.value, false),
            ExprKind::VarDecl(e) => {
                self.lower_decl_expr(e.name.symbol, Some(&e.typ), &e.expr, true)
            }
            ExprKind::Set(e) => self.lower_set_expr(e),
            ExprKind::Id(e) => self.lower_id_expr(expr.span.clone(), e),
            ExprKind::Block(e) => self.lower_block_expr(e),
            ExprKind::CompareChain(e) => self.lower_compare_chain_expr(expr.span.clone(), e),
            ExprKind::Template(e) => self.lower_template_expr(e),
            ExprKind::Tuple(e) => self.lower_tuple_expr(e),
            ExprKind::If(e) => self.lower_if_expr(e),
            ExprKind::Loop(body) => self.lower_loop_expr(body),
            ExprKind::Break => self.lower_break_expr(expr),
            ExprKind::Func(e) => self.lower_func_expr(e),
            ExprKind::Call(e) => self.lower_call_expr(expr.span.clone(), e),
            ExprKind::Binary(e) => self.lower_binary_expr(expr.span.clone(), e),
            ExprKind::Unary(e) => self.lower_unary_expr(e),
            ExprKind::Type(e) => {
                let type_info = self.lower_type_expr(e);
                Some(Ir {
                    kind: IrKind::Type(type_info.clone()),
                    ty: TypeInfo::Type(type_info.into()),
                })
            }
            ExprKind::Member(expr) => self.lower_member_expr(expr),
            ExprKind::Construct(cons_expr) => self.lower_construct_expr(cons_expr),
        }
    }

    fn lower_logic_expr(&mut self, value: bool) -> Ir {
        if value {
            Ir {
                kind: IrKind::Logic(value),
                ty: TypeInfo::Logic,
            }
        } else {
            Ir {
                kind: IrKind::Logic(value),
                ty: TypeInfo::False,
            }
        }
    }

    fn lower_decl_expr(
        &mut self,
        name: Symbol,
        ty: Option<&TypeExpr>,
        value: &Expression,
        mutable: bool,
    ) -> Option<Ir> {
        let mut value_ir = self.lower_expr(value)?;
        let mut binding_type = value_ir.ty.clone();

        if let Some(type_expr) = ty
            && !matches!(type_expr.kind, TypeExprKind::Type)
        {
            binding_type = self.lower_type_expr(type_expr);

            if !self.is_assignable_to(&value_ir.ty, &binding_type) {
                self.errors.push(SemanticError::TypeMismatch {
                    span: value.span.clone(),
                    expected: binding_type.clone(),
                    found: value_ir.ty.clone(),
                })
            }

            if matches!(type_expr.kind, TypeExprKind::Option(_)) && value_ir.ty == TypeInfo::False {
                value_ir = Ir {
                    kind: IrKind::Option(None),
                    ty: binding_type.clone(),
                }
            }
        }

        let slot = self.declare(name, binding_type.clone(), mutable);

        Some(Ir {
            kind: IrKind::StoreLocal {
                slot,
                value: Box::new(value_ir),
            },
            ty: binding_type,
        })
    }

    fn lower_set_expr(&mut self, expr: &SetExpr) -> Option<Ir> {
        let value = self.lower_expr(&expr.rhs)?;

        match &expr.lhs.kind {
            ExprKind::Id(id_expr) => {
                let var = match self.lookup(&id_expr.symbol) {
                    Some(var) => var,
                    None => {
                        self.errors.push(SemanticError::UndefinedName {
                            span: expr.lhs.span.clone(),
                        });
                        return None;
                    }
                };

                if var.type_info != value.ty {
                    self.errors.push(SemanticError::TypeMismatch {
                        span: expr.rhs.span.clone(),
                        expected: var.type_info.clone(),
                        found: value.ty.clone(),
                    })
                }
                if !var.mutable {
                    self.errors.push(SemanticError::ImmutableAssignment {
                        span: expr.lhs.span.clone(),
                        symbol: id_expr.symbol,
                    })
                }

                let ir = if var.is_global {
                    Ir {
                        kind: IrKind::StoreGlobal {
                            slot: var.slot,
                            value: value.into(),
                        },
                        ty: var.type_info,
                    }
                } else if var.is_upvalue {
                    Ir {
                        kind: IrKind::StoreUpvalue {
                            index: self.capture(var.scope_index, var.slot),
                            value: value.into(),
                        },
                        ty: var.type_info,
                    }
                } else {
                    Ir {
                        kind: IrKind::StoreLocal {
                            slot: var.slot,
                            value: value.into(),
                        },
                        ty: var.type_info,
                    }
                };
                Some(ir)
            }
            _ => {
                self.errors.push(SemanticError::InvalidAssignmentTarget {
                    span: expr.lhs.span.clone(),
                });
                None
            }
        }
    }

    fn lower_id_expr(&mut self, span: Span, expr: &IdExpr) -> Option<Ir> {
        if let Some(var) = self.lookup(&expr.symbol) {
            let ir = if var.is_global {
                Ir {
                    kind: IrKind::LoadGlobal { slot: var.slot },
                    ty: var.type_info,
                }
            } else if var.is_upvalue {
                let index = self.capture(var.scope_index, var.slot);
                Ir {
                    kind: IrKind::LoadUpvalue { index },
                    ty: var.type_info,
                }
            } else {
                Ir {
                    kind: IrKind::LoadLocal { slot: var.slot },
                    ty: var.type_info,
                }
            };
            Some(ir)
        } else {
            self.errors.push(SemanticError::UndefinedName { span });
            None
        }
    }

    fn lower_block_expr(&mut self, expr: &BlockExpr) -> Option<Ir> {
        let body: Vec<_> = expr.body.iter().map(|e| self.lower_expr(e)).collect();

        if let Some(last_ir) = body.last()
            && last_ir.is_none()
        {
            return None;
        }

        let body: Vec<_> = body.into_iter().flatten().collect();

        Some(Ir {
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

        self.ensure_not_fallible(span.clone());

        let head_ir = self.lower_expr(&expr.head)?;

        let mut rest_irs = vec![];
        for (_, expr) in &expr.rest {
            let ir = self.lower_expr(expr)?;
            if ir.ty != head_ir.ty {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.span.clone(),
                    expected: head_ir.ty.clone(),
                    found: ir.ty.clone(),
                });
            }
            rest_irs.push(ir);
        }

        Some(Ir {
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

    fn lower_template_expr(&mut self, expr: &TemplateExpression) -> Option<Ir> {
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
            kind: IrKind::Template(elements),
            ty: TypeInfo::String,
        })
    }

    fn lower_tuple_expr(&mut self, expr: &TupleExpr) -> Option<Ir> {
        let mut types = Vec::with_capacity(expr.elements.len());
        let mut irs = Vec::with_capacity(expr.elements.len());

        for el in expr.elements.iter() {
            let ir = self.lower_expr(el)?;
            types.push(ir.ty.clone());
            irs.push(ir);
        }

        Some(Ir {
            kind: IrKind::Tuple(irs),
            ty: TypeInfo::Tuple(types),
        })
    }

    fn lower_if_expr(&mut self, expr: &IfExpr) -> Option<Ir> {
        self.push_scope(false);
        self.push_failure_context();
        let test_ir = self.lower_expr(&expr.test);
        self.pop_failure_context();
        let then_ir = self.lower_expr(&expr.consequent);
        self.pop_scope();

        let test_ir = test_ir?;

        if !test_ir.kind.is_fallible() {
            self.errors.push(SemanticError::ExpectedFallibleExpr {
                span: expr.test.span.clone(),
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
            kind: IrKind::If(IfIr {
                test: test_ir.into(),
                then: then_ir.into(),
                alt: alt_ir.map(|ir| ir.into()),
            }),
            ty: expr_type,
        })
    }

    fn lower_loop_expr(&mut self, body: &Expression) -> Option<Ir> {
        self.push_loop();
        let body_ir = self.lower_expr(body);
        self.pop_loop();
        Some(Ir {
            ty: TypeInfo::True,
            kind: IrKind::Loop(body_ir?.into()),
        })
    }

    fn lower_break_expr(&mut self, expr: &Expression) -> Option<Ir> {
        if self.loop_stack.is_empty() {
            self.errors.push(SemanticError::BreakOutsideLoop {
                span: expr.span.clone(),
            });
            return None;
        }

        Some(Ir {
            ty: TypeInfo::Bottom,
            kind: IrKind::Break,
        })
    }

    fn lower_func_expr(&mut self, expr: &FunctionExpr) -> Option<Ir> {
        let return_type = self.lower_type_expr(&expr.return_type);
        let param_names: Vec<_> = expr.params.iter().map(|p| p.name).collect();
        let param_types: Vec<_> = expr
            .params
            .iter()
            .map(|p| self.lower_type_expr(&p.typ))
            .collect();

        let mut effects = Effects { decides: false };
        for effect in expr.effects.iter() {
            match effect.symbol {
                e if e == self.builtin_symbols.s_decides => {
                    effects.decides = true;
                }
                _ => self.errors.push(SemanticError::InvalidEffect {
                    span: effect.span.clone(),
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

        let body = body?;

        if return_type != TypeInfo::Void {
            if !self.is_assignable_to(&body.ty, &return_type) {
                self.errors.push(SemanticError::TypeMismatch {
                    span: expr.body.span.clone(),
                    expected: return_type.clone(),
                    found: body.ty.clone(),
                })
            }
        }

        let return_void = return_type == TypeInfo::Void;
        let type_info = TypeInfo::Function {
            params: param_types,
            ret: return_type.into(),
        };

        let upvalues = scope.upvalues.into_iter().collect();
        let func_slot = self.declare(expr.name, type_info.clone(), false);

        Some(Ir {
            kind: IrKind::Func(FunctionIr {
                slot: func_slot,
                params: param_slots,
                effects,
                body: body.into(),
                return_void,
                upvalues,
            }),
            ty: type_info,
        })
    }

    fn lower_type_cast(
        &mut self,
        span: Span,
        args: &[Expression],
        type_info: &TypeInfo,
    ) -> Option<Ir> {
        self.ensure_not_fallible(span.clone());

        if args.len() != 1 {
            self.errors.push(SemanticError::ArgCountMismatch {
                span,
                expected: 1,
                found: args.len(),
            });
        }

        if let Some(arg) = args.first() {
            let arg = self.lower_expr(arg)?;
            Some(Ir {
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
        let callee_ir = self.lower_expr(&expr.callee)?;

        let mut arg_irs = vec![];
        match &callee_ir.ty {
            TypeInfo::Function { params, ret } => {
                if params.len() != expr.args.len() {
                    self.errors.push(SemanticError::ArgCountMismatch {
                        span: expr.callee.span.clone(),
                        expected: params.len(),
                        found: expr.args.len(),
                    })
                }
                for (param_type, arg) in params.iter().zip(expr.args.iter()) {
                    let arg_ir = self.lower_expr(arg)?;
                    if !self.is_assignable_to(&arg_ir.ty, param_type) {
                        self.errors.push(SemanticError::TypeMismatch {
                            span: arg.span.clone(),
                            expected: param_type.clone(),
                            found: arg_ir.ty.clone(),
                        })
                    }
                    arg_irs.push(arg_ir);
                }
                let return_type = ret.clone();
                Some(Ir {
                    kind: IrKind::Call(CallIr {
                        callee: callee_ir.into(),
                        args: arg_irs,
                    }),
                    ty: *return_type,
                })
            }
            TypeInfo::Tuple(elements) => {
                if expr.args.len() != 1 {
                    self.errors.push(SemanticError::ArgCountMismatch {
                        span: span.clone(),
                        expected: 1,
                        found: expr.args.len(),
                    });
                }
                if let Some(arg) = expr.args.first() {
                    let arg = self.lower_expr(arg)?;
                    if let IrKind::Int(index) = arg.kind {
                        if index < 0 || index as usize >= elements.len() {
                            self.errors.push(SemanticError::TupleIndexOutOfBounds {
                                span: expr.args[0].span.clone(),
                                index,
                                length: elements.len(),
                            });
                            None
                        } else {
                            let type_info = elements[index as usize].clone();
                            Some(Ir {
                                kind: IrKind::IndexTuple {
                                    tuple: callee_ir.into(),
                                    index: index as usize,
                                },
                                ty: type_info,
                            })
                        }
                    } else {
                        self.errors.push(SemanticError::InvalidTupleIndex {
                            span: expr.args[0].span.clone(),
                        });
                        None
                    }
                } else {
                    None
                }
            }
            TypeInfo::Type(type_id) => self.lower_type_cast(span, &expr.args, type_id),
            _ => {
                self.errors.push(SemanticError::NotCallable {
                    span: expr.callee.span.clone(),
                    ty: callee_ir.ty.clone(),
                });
                None
            }
        }
    }

    fn lower_binary_expr(&mut self, span: Span, expr: &BinaryExpr) -> Option<Ir> {
        let lhs = self.lower_expr(&expr.lhs)?;
        let rhs = self.lower_expr(&expr.rhs)?;

        if lhs.ty == TypeInfo::String {
            if expr.op == BinaryOp::Add {
                if rhs.ty == lhs.ty {
                    let mut irs = vec![];
                    flatten_add_ir(lhs, &mut irs);
                    flatten_add_ir(rhs, &mut irs);
                    return Some(Ir {
                        ty: TypeInfo::String,
                        kind: IrKind::Concat(irs),
                    });
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        span,
                        expected: lhs.ty,
                        found: rhs.ty,
                    });
                    return None;
                }
            } else {
                self.errors.push(SemanticError::InvalidBinaryOp {
                    span: expr.op_span.clone(),
                    op: expr.op,
                    lhs: lhs.ty.clone(),
                    rhs: rhs.ty.clone(),
                });
                return None;
            }
        }

        let type_id = if lhs.ty == rhs.ty {
            lhs.ty.clone()
        } else {
            self.errors.push(SemanticError::TypeMismatch {
                span: span.clone(),
                expected: lhs.ty.clone(),
                found: rhs.ty.clone(),
            });
            TypeInfo::Any
        };

        let kind = match expr.op {
            BinaryOp::Add => IrKind::Add((lhs.into(), rhs.into())),
            BinaryOp::Sub => IrKind::Sub((lhs.into(), rhs.into())),
            BinaryOp::Mul => IrKind::Mul((lhs.into(), rhs.into())),
            BinaryOp::Div => IrKind::Div((lhs.into(), rhs.into())),
        };

        Some(Ir { kind, ty: type_id })
    }

    fn lower_unary_expr(&mut self, expr: &UnaryExpr) -> Option<Ir> {
        let ir = self.lower_expr(&expr.expr)?;
        match expr.op {
            UnaryOp::Plus => Some(ir),
            UnaryOp::Minus => {
                let expected_types = vec![TypeInfo::Int, TypeInfo::Float, TypeInfo::Rational];
                if expected_types.contains(&ir.ty) {
                    Some(Ir {
                        ty: ir.ty.clone(),
                        kind: IrKind::Neg(ir.into()),
                    })
                } else {
                    self.errors.push(SemanticError::InvalidUnaryOp {
                        span: expr.expr.span.clone(),
                        op: expr.op,
                        operand: ir.ty,
                        expected: expected_types,
                    });
                    None
                }
            }
            UnaryOp::Not => Some(Ir {
                ty: TypeInfo::Logic,
                kind: IrKind::Not(ir.into()),
            }),
        }
    }

    fn lower_type_expr(&mut self, expr: &TypeExpr) -> TypeInfo {
        let type_id = match &expr.kind {
            TypeExprKind::Named(symbol) => (|| -> TypeInfo {
                if let Some(binding) = self.lookup(symbol) {
                    if let TypeInfo::Type(inner_type) = binding.type_info {
                        return *inner_type;
                    } else {
                        self.errors.push(SemanticError::ExpectedTypeGotValue {
                            span: expr.span.clone(),
                        });
                        return TypeInfo::Any;
                    }
                }
                self.errors.push(SemanticError::UndefinedName {
                    span: expr.span.clone(),
                });
                TypeInfo::Any
            })(),
            TypeExprKind::Option(inner) => {
                let inner = self.lower_type_expr(inner);
                TypeInfo::Option(inner.into())
            }
            TypeExprKind::Tuple(args) => {
                let mut arg_ids = vec![];
                for arg in args {
                    arg_ids.push(self.lower_type_expr(arg));
                }
                TypeInfo::Tuple(arg_ids)
            }
            TypeExprKind::Array(elem_type) => {
                let elem_type_id = self.lower_type_expr(elem_type);
                TypeInfo::Array(elem_type_id.into())
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> = params.iter().map(|p| self.lower_type_expr(p)).collect();
                let ret_ty = self.lower_type_expr(ret);
                TypeInfo::Function {
                    params: param_types,
                    ret: ret_ty.into(),
                }
            }
            TypeExprKind::Type => {
                panic!("{:?}", expr);
                // self.builtin_types.t_any
            }
        };

        type_id
    }

    fn lower_member_expr(&mut self, expr: &MemberExpr) -> Option<Ir> {
        let obj = self.lower_expr(&expr.object)?;

        if matches!(obj.ty, TypeInfo::String | TypeInfo::Array(_)) {
            if let ExprKind::Id(id_expr) = &expr.property.kind {
                if id_expr.symbol == self.builtin_symbols.s_Length {
                    return Some(Ir {
                        kind: IrKind::GetLength(obj.into()),
                        ty: TypeInfo::Int,
                    });
                }
            }
        }

        None
    }

    fn lower_construct_expr(&mut self, cons_expr: &ConstructExpr) -> Option<Ir> {
        if let ExprKind::Id(id_expr) = &cons_expr.callee.kind {
            if id_expr.symbol == self.builtin_symbols.s_option {
                return self.lower_construct_option(cons_expr);
            }
            if id_expr.symbol == self.builtin_symbols.s_array {
                return self.lower_construct_array(cons_expr);
            }
        }

        todo!()
    }

    fn lower_construct_option(&mut self, cons_expr: &ConstructExpr) -> Option<Ir> {
        if let Some(value) = cons_expr.args.first() {
            let value = self.lower_expr(value)?;
            return Some(Ir {
                ty: TypeInfo::Option(value.ty.clone().into()),
                kind: IrKind::Option(Some(value.into())),
            });
        }
        None
    }

    fn lower_construct_array(&mut self, cons_expr: &ConstructExpr) -> Option<Ir> {
        if cons_expr.args.is_empty() {
            todo!();
        }

        let mut irs: Vec<Ir> = vec![];
        for (i, arg) in cons_expr.args.iter().enumerate() {
            let ir = self.lower_expr(arg)?;
            if i > 0 {
                let head = &irs[0];
                if !self.is_assignable_to(&ir.ty, &head.ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        span: arg.span.clone(),
                        expected: head.ty.clone(),
                        found: ir.ty.clone(),
                    });
                }
            }
            irs.push(ir);
        }

        Some(Ir {
            ty: TypeInfo::Array(irs.first().unwrap().ty.clone().into()),
            kind: IrKind::Array(irs),
        })
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

#[derive(Debug)]
pub enum SemanticError {
    UndefinedName {
        span: Span,
    },

    TypeMismatch {
        span: Span,
        expected: TypeInfo,
        found: TypeInfo,
    },

    InvalidUnaryOp {
        span: Span,
        op: UnaryOp,
        operand: TypeInfo,
        expected: Vec<TypeInfo>,
    },

    InvalidBinaryOp {
        span: Span,
        op: BinaryOp,
        lhs: TypeInfo,
        rhs: TypeInfo,
    },

    ImmutableAssignment {
        span: Span,
        symbol: Symbol,
    },

    InvalidAssignmentTarget {
        span: Span,
    },

    NotCallable {
        span: Span,
        ty: TypeInfo,
    },

    ArgCountMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },

    InvalidTupleIndex {
        span: Span,
    },

    TupleIndexOutOfBounds {
        span: Span,
        index: i64,
        length: usize,
    },

    ExpectedTypeGotValue {
        span: Span,
    },

    BreakOutsideLoop {
        span: Span,
    },

    InvalidEffect {
        span: Span,
    },

    UnexpectedFallibleExpr {
        span: Span,
    },

    ExpectedFallibleExpr {
        span: Span,
    },
}
