use ordermap::OrderSet;
use std::collections::HashMap;
use thiserror::Error;

use super::ast::*;
use super::ir::{self, Ir, Slot, UpvalueDesc};
use super::lexer::Span;
use crate::{
    core::{
        PredefinedSymbols, Symbol, SymbolRegistry,
        types::{PredefinedTypes, TypeInfo, TypeRegistry},
    },
    runtime::{FnKind, TypeId, Value, builtin_funcs},
};

#[derive(Debug, Clone, Copy)]
pub struct Variable {
    pub slot: Slot,
    pub type_id: TypeId,
    pub mutable: bool,
}

struct LookupResult {
    is_global: bool,
    is_upvalue: bool,
    slot: Slot,
    type_id: TypeId,
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

    fn declare(&mut self, symbol: Symbol, type_id: TypeId, mutable: bool) -> Slot {
        if let Some(binding) = self.variables.get_mut(&symbol) {
            binding.type_id = type_id;
            binding.mutable = mutable;
            return binding.slot;
        }
        let slot = Slot(self.next_slot);
        self.next_slot += 1;
        self.variables.insert(
            symbol,
            Variable {
                slot,
                type_id,
                mutable,
            },
        );
        slot
    }

    pub fn lookup(&self, symbol: Symbol) -> Option<&Variable> {
        self.variables.get(&symbol)
    }
}

struct LoopInfo {}

pub struct SemanticAnalyzer {
    pub builtin_symbols: PredefinedSymbols,
    pub predefined_types: PredefinedTypes,
    pub errors: Vec<SemanticError>,

    pub scopes: Vec<Scope>,
    pub types: TypeRegistry,

    loop_stack: Vec<LoopInfo>,
}

impl SemanticAnalyzer {
    pub fn new(symbol_table: &mut SymbolRegistry) -> Self {
        let mut global_scope = Scope::new(false);
        let mut types = TypeRegistry::default();

        let bs = PredefinedSymbols::install(symbol_table);
        let bt = PredefinedTypes::install(&mut types);

        let predefined_vars = [
            (bs.s_int, TypeInfo::Type(bt.t_int)),
            (bs.s_float, TypeInfo::Type(bt.t_float)),
            (bs.s_logic, TypeInfo::Type(bt.t_logic)),
            (bs.s_char, TypeInfo::Type(bt.t_char)),
            (bs.s_char32, TypeInfo::Type(bt.t_char32)),
            (bs.s_string, TypeInfo::Type(bt.t_string)),
            (bs.s_any, TypeInfo::Type(bt.t_any)),
            (bs.s_void, TypeInfo::Type(bt.t_void)),
            (bs.s_Print, TypeInfo::Any),
        ];

        for (s, t) in predefined_vars {
            global_scope.declare(s, types.intern(t), false);
        }

        Self {
            scopes: vec![global_scope],
            builtin_symbols: bs,
            predefined_types: bt,
            errors: vec![],
            types,
            loop_stack: vec![],
        }
    }

    pub fn get_global_vars(&self) -> Vec<Value> {
        let ps = self.builtin_symbols;
        let pt = self.predefined_types;

        let global_bindings = [
            (ps.s_int, Value::Type(pt.t_int)),
            (ps.s_float, Value::Type(pt.t_float)),
            (ps.s_logic, Value::Type(pt.t_logic)),
            (ps.s_char, Value::Type(pt.t_char)),
            (ps.s_char32, Value::Type(pt.t_char32)),
            (ps.s_string, Value::Type(pt.t_string)),
            (ps.s_any, Value::Type(pt.t_any)),
            (ps.s_void, Value::Type(pt.t_void)),
            (
                ps.s_Print,
                Value::Function {
                    kind: FnKind::Native(builtin_funcs::print),
                },
            ),
        ];

        let mut global_vars: Vec<_> = global_bindings
            .into_iter()
            .map(|(symbol, value)| (self.scopes[0].lookup(symbol).unwrap().slot.0, value))
            .collect();

        global_vars.sort_by(|a, b| a.0.cmp(&b.0));
        global_vars.into_iter().map(|(_, v)| v).collect()
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

    pub fn declare(&mut self, symbol: Symbol, type_id: TypeId, mutable: bool) -> Slot {
        // TODO: check shadowing
        self.scopes
            .last_mut()
            .unwrap()
            .declare(symbol, type_id, mutable)
    }

    fn lookup(&mut self, symbol: &Symbol) -> Option<LookupResult> {
        let mut captured = false;
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(var) = scope.lookup(*symbol) {
                let res = LookupResult {
                    is_global: index == 0 && self.scopes.len() > 1,
                    is_upvalue: captured,
                    slot: var.slot,
                    type_id: var.type_id,
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

    fn is_assignable_to(&self, from: TypeId, to: TypeId) -> bool {
        if from == to || to == self.predefined_types.t_any {
            return true;
        }

        let from = self.types.lookup(from).unwrap();
        let to = self.types.lookup(to).unwrap();

        match (from, to) {
            (TypeInfo::True | TypeInfo::False, TypeInfo::Logic) => true,
            (TypeInfo::False, TypeInfo::Option(_)) => true,
            (TypeInfo::Option(from), TypeInfo::Option(to)) => self.is_assignable_to(*from, *to),
            _ => false,
        }
    }

    fn push_loop(&mut self) {
        self.loop_stack.push(LoopInfo {});
    }

    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    fn emit_type_mismatch_error(&mut self, span: Span, expect: TypeId, found: TypeId) {
        self.errors.push(SemanticError::TypeMismatch {
            span,
            expect,
            found,
        });
    }

    pub fn analyze(&mut self, program: &[Expression]) -> Vec<Ir> {
        let mut root_irs = vec![];
        for expr in program {
            root_irs.push(self.handle_expr(expr));
        }
        root_irs
    }

    pub fn handle_expr(&mut self, expr: &Expression) -> Ir {
        match &expr.kind {
            ExprKind::Integer(v) => Ir {
                kind: ir::ExprKind::Int(*v),
                ty: self.predefined_types.t_int,
            },
            ExprKind::Float(v) => Ir {
                kind: ir::ExprKind::Float(*v),
                ty: self.predefined_types.t_float,
            },
            ExprKind::Logic(v) => self.handle_logic_expr(*v),
            ExprKind::Char(v) => Ir {
                kind: ir::ExprKind::Char(*v),
                ty: self.predefined_types.t_char,
            },
            ExprKind::Char32(v) => Ir {
                kind: ir::ExprKind::Char32(*v),
                ty: self.predefined_types.t_char32,
            },
            ExprKind::String(v) => Ir {
                kind: ir::ExprKind::String(*v),
                ty: self.predefined_types.t_string,
            },
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
            ExprKind::Loop(body) => self.handle_loop_expr(body),
            ExprKind::Break => self.handle_break_expr(expr),
            ExprKind::Func(e) => self.handle_func_expr(e),
            ExprKind::Call(e) => self.handle_call_expr(expr.span.clone(), e),
            ExprKind::Binary(e) => self.handle_binary_expr(expr.span.clone(), e),
            ExprKind::Unary(e) => self.handle_unary_expr(e),
            ExprKind::Type(e) => {
                let type_id = self.handle_type_expr(e);
                Ir {
                    kind: ir::ExprKind::Type(type_id),
                    ty: type_id,
                }
            }
            ExprKind::Member(expr) => self.handle_member_expr(expr),
            ExprKind::Construct(cons_expr) => self.handle_construct_expr(cons_expr),
        }
    }

    fn placeholder_ir(&self) -> Ir {
        Ir {
            kind: ir::ExprKind::Nop,
            ty: self.predefined_types.t_any,
        }
    }

    fn handle_logic_expr(&mut self, value: bool) -> Ir {
        if value {
            Ir {
                kind: ir::ExprKind::Logic(value),
                ty: self.predefined_types.t_logic,
            }
        } else {
            Ir {
                kind: ir::ExprKind::Logic(value),
                ty: self.predefined_types.t_false,
            }
        }
    }

    fn handle_decl_option(
        &mut self,
        name: Symbol,
        ty: TypeId,
        value: &Expression,
        mutable: bool,
    ) -> Ir {
        let value_ir = self.handle_expr(value);

        let slot = self.declare(name, ty, mutable);

        if value_ir.ty == self.predefined_types.t_false {
            Ir {
                kind: ir::ExprKind::StoreLocal {
                    slot,
                    value: Box::new(Ir {
                        kind: ir::ExprKind::Option(None),
                        ty,
                    }),
                },
                ty,
            }
        } else if self.is_assignable_to(value_ir.ty, ty) {
            Ir {
                kind: ir::ExprKind::StoreLocal {
                    slot,
                    value: Box::new(value_ir),
                },
                ty,
            }
        } else {
            self.errors.push(SemanticError::TypeMismatch {
                span: value.span.clone(),
                expect: ty,
                found: value_ir.ty,
            });
            self.placeholder_ir()
        }
    }

    fn handle_decl_expr(
        &mut self,
        name: Symbol,
        ty: Option<&TypeExpr>,
        value: &Expression,
        mutable: bool,
    ) -> Ir {
        let (value_ir, binding_type) = if let Some(typ) = ty
            && !matches!(typ.kind, TypeExprKind::Type)
        {
            let decl_type = self.handle_type_expr(typ);
            if let TypeExprKind::Option(_) = typ.kind {
                return self.handle_decl_option(name, decl_type, value, mutable);
            }

            let value_ir = self.handle_expr(value);
            if !self.is_assignable_to(value_ir.ty, decl_type) {
                self.emit_type_mismatch_error(value.span.clone(), decl_type, value_ir.ty);
            }
            (value_ir, decl_type)
        } else {
            let value_ir = self.handle_expr(value);
            let value_ty = value_ir.ty;
            (value_ir, value_ty)
        };

        let slot = self.declare(name, binding_type, mutable);

        Ir {
            kind: ir::ExprKind::StoreLocal {
                slot,
                value: Box::new(value_ir),
            },
            ty: binding_type,
        }
    }

    fn handle_set_expr(&mut self, expr: &SetExpr) -> Ir {
        let value_ir = self.handle_expr(&expr.expr);
        let value_type = value_ir.ty;

        match &expr.target.kind {
            LValueKind::Id(id_expr) => {
                if let Some(var) = self.lookup(&id_expr.symbol) {
                    if var.type_id != value_type {
                        self.emit_type_mismatch_error(
                            expr.expr.span.clone(),
                            var.type_id,
                            value_type,
                        );
                    }
                    if !var.mutable {
                        self.errors.push(SemanticError::Mutability {
                            span: expr.target.span.clone(),
                            symbol: id_expr.symbol,
                        })
                    }
                    if var.is_global {
                        Ir {
                            kind: ir::ExprKind::StoreGlobal {
                                slot: var.slot,
                                value: value_ir.into(),
                            },
                            ty: var.type_id,
                        }
                    } else if var.is_upvalue {
                        Ir {
                            kind: ir::ExprKind::StoreUpvalue {
                                index: self.capture(var.scope_index, var.slot),
                                value: value_ir.into(),
                            },
                            ty: var.type_id,
                        }
                    } else {
                        Ir {
                            kind: ir::ExprKind::StoreLocal {
                                slot: var.slot,
                                value: value_ir.into(),
                            },
                            ty: var.type_id,
                        }
                    }
                } else {
                    self.placeholder_ir()
                }
            }
        }
    }

    fn handle_id_expr(&mut self, span: Span, expr: &IdExpr) -> Ir {
        if let Some(var) = self.lookup(&expr.symbol) {
            if var.is_global {
                Ir {
                    kind: ir::ExprKind::LoadGlobal { slot: var.slot },
                    ty: var.type_id,
                }
            } else if var.is_upvalue {
                let index = self.capture(var.scope_index, var.slot);
                Ir {
                    kind: ir::ExprKind::LoadUpvalue { index },
                    ty: var.type_id,
                }
            } else {
                Ir {
                    kind: ir::ExprKind::LoadLocal { slot: var.slot },
                    ty: var.type_id,
                }
            }
        } else {
            self.errors.push(SemanticError::Reference {
                span,
                symbol: expr.symbol,
            });
            self.placeholder_ir()
        }
    }

    fn handle_block_expr(&mut self, expr: &BlockExpr) -> Ir {
        let body: Vec<_> = expr
            .body
            .iter()
            .map(|expr| self.handle_expr(expr))
            .collect();

        let type_id = body
            .last()
            .map(|ar| ar.ty)
            .unwrap_or(self.predefined_types.t_void);

        Ir {
            kind: ir::ExprKind::Block(body),
            ty: type_id,
        }
    }

    fn handle_compare_chain_expr(&mut self, expr: &CompareChainExpr) -> Ir {
        // TODO: check if items are comparable
        // Currently just check if they are the same type

        let head_ar = self.handle_expr(&expr.head);
        let head_type = head_ar.ty;

        let mut rest_irs = vec![];
        for (_, expr) in &expr.rest {
            let ir = self.handle_expr(expr);
            if ir.ty != head_type {
                self.emit_type_mismatch_error(expr.span.clone(), head_type, ir.ty);
            }
            rest_irs.push(ir);
        }

        Ir {
            kind: ir::ExprKind::CompareChain(ir::CompareChainExpr {
                head: head_ar.into(),
                rest: rest_irs
                    .into_iter()
                    .zip(expr.rest.iter())
                    .map(|(ir, (op, _))| (*op, ir))
                    .collect(),
            }),
            ty: head_type,
        }
    }

    fn handle_template_expr(&mut self, expr: &TemplateExpression) -> Ir {
        let elements: Vec<_> = expr
            .elements
            .iter()
            .map(|el| match el {
                TemplateElement::Expr(expr) => {
                    ir::TemplateElement::Expr(self.handle_expr(expr).into())
                }
                TemplateElement::Raw(const_id) => ir::TemplateElement::String(*const_id),
            })
            .collect();

        Ir {
            kind: ir::ExprKind::Template(elements),
            ty: self.predefined_types.t_string,
        }
    }

    fn handle_tuple_expr(&mut self, expr: &TupleExpr) -> Ir {
        let (elem_types, elem_irs): (Vec<_>, Vec<_>) = expr
            .elements
            .iter()
            .map(|el| {
                let ir = self.handle_expr(el);
                (ir.ty, ir)
            })
            .unzip();

        let type_id = self.types.intern(TypeInfo::Tuple(elem_types));

        Ir {
            kind: ir::ExprKind::Tuple(elem_irs),
            ty: type_id,
        }
    }

    fn handle_if_expr(&mut self, expr: &IfExpr) -> Ir {
        self.push_scope(false);
        let test_ir = self.handle_expr(&expr.test);
        let then_ir = self.handle_expr(&expr.consequent);
        self.pop_scope();

        let (expr_type, alt_ar) = if let Some(alt) = &expr.alternate {
            self.push_scope(false);
            let alt_ir = self.handle_expr(alt);
            self.pop_scope();
            let expr_type = if then_ir.ty != alt_ir.ty {
                self.predefined_types.t_any
            } else {
                then_ir.ty
            };
            (expr_type, Some(alt_ir))
        } else {
            (self.types.intern(TypeInfo::Option(then_ir.ty)), None)
        };

        Ir {
            kind: ir::ExprKind::If(ir::IfExpr {
                test: test_ir.into(),
                then: then_ir.into(),
                alt: alt_ar.map(|ar| ar.into()),
            }),
            ty: expr_type,
        }
    }

    fn handle_loop_expr(&mut self, body: &Expression) -> Ir {
        self.push_loop();
        let body_ir = self.handle_expr(body);
        let ir = Ir {
            ty: self.predefined_types.t_true,
            kind: ir::ExprKind::Loop(body_ir.into()),
        };
        self.pop_loop();
        ir
    }

    fn handle_break_expr(&mut self, expr: &Expression) -> Ir {
        if self.loop_stack.is_empty() {
            self.errors.push(SemanticError::BreakOutsideLoop {
                span: expr.span.clone(),
            });
            self.placeholder_ir()
        } else {
            Ir {
                ty: self.predefined_types.t_bottom,
                kind: ir::ExprKind::Break,
            }
        }
    }

    fn handle_func_expr(&mut self, expr: &FunctionExpr) -> Ir {
        let return_type = self.handle_type_expr(&expr.return_type);
        let param_names: Vec<_> = expr.params.iter().map(|p| p.name).collect();
        let param_types: Vec<_> = expr
            .params
            .iter()
            .map(|p| self.handle_type_expr(&p.typ))
            .collect();

        self.push_scope(true);

        let mut param_slots = vec![];
        for (param_name, param_type) in param_names.iter().zip(param_types.iter()) {
            let slot = self.declare(*param_name, *param_type, true);
            param_slots.push(slot);
        }

        let body = self.handle_expr(&expr.body);

        if return_type != self.predefined_types.t_void {
            if !self.is_assignable_to(body.ty, return_type) {
                self.emit_type_mismatch_error(expr.body.span.clone(), return_type, body.ty);
            }
        }

        let type_id = self.types.intern(TypeInfo::Function {
            params: param_types,
            ret: return_type,
        });

        let scope = self.pop_scope();
        let upvalues = scope.upvalues.into_iter().collect();
        let func_slot = self.declare(expr.name, type_id, false);

        Ir {
            kind: ir::ExprKind::Func(ir::FunctionExpr {
                slot: func_slot,
                params: param_slots,
                body: body.into(),
                return_void: return_type == self.predefined_types.t_void,
                upvalues,
            }),
            ty: type_id,
        }
    }

    fn handle_type_cast(&mut self, span: Span, args: &[Expression], type_id: TypeId) -> Ir {
        if args.len() == 1 {
            let arg = self.handle_expr(&args[0]);
            Ir {
                kind: ir::ExprKind::Cast {
                    ty: type_id,
                    value: arg.into(),
                },
                ty: type_id,
            }
        } else {
            self.errors.push(SemanticError::ArgsCountMismatch { span });
            self.placeholder_ir()
        }
    }

    fn handle_call_expr(&mut self, span: Span, expr: &CallExpr) -> Ir {
        let callee_ar = self.handle_expr(&expr.callee);

        let mut arg_hir_ids = vec![];
        match self.types.lookup(callee_ar.ty).cloned().unwrap() {
            TypeInfo::Function { params, ret } => {
                if params.len() != expr.args.len() {
                    self.errors.push(SemanticError::ArgsCountMismatch {
                        span: expr.callee.span.clone(),
                    })
                }
                for (&param_type, arg) in params.iter().zip(expr.args.iter()) {
                    let arg_ar = self.handle_expr(arg);
                    if !self.is_assignable_to(arg_ar.ty, param_type) {
                        self.emit_type_mismatch_error(arg.span.clone(), param_type, arg_ar.ty);
                    }
                    arg_hir_ids.push(arg_ar);
                }
                Ir {
                    kind: ir::ExprKind::Call(ir::CallExpr {
                        callee: callee_ar.into(),
                        args: arg_hir_ids,
                    }),
                    ty: ret,
                }
            }
            TypeInfo::Any => {
                // TODO: handle builtin functions
                for arg in &expr.args {
                    arg_hir_ids.push(self.handle_expr(arg));
                }
                Ir {
                    kind: ir::ExprKind::Call(ir::CallExpr {
                        callee: callee_ar.into(),
                        args: arg_hir_ids,
                    }),
                    ty: self.predefined_types.t_any,
                }
            }
            TypeInfo::Tuple(elements) => {
                if expr.args.len() == 1 {
                    let arg = self.handle_expr(&expr.args[0]);
                    if let ir::ExprKind::Int(index) = arg.kind {
                        Ir {
                            kind: ir::ExprKind::IndexTuple {
                                tuple: callee_ar.into(),
                                index: index as usize,
                            },
                            ty: elements[index as usize],
                        }
                    } else {
                        self.errors.push(SemanticError::UnexpectedExpr {
                            span: expr.args[0].span.clone(),
                            expect: "integer".to_string(),
                            found: format!("{:?}", expr.args[0]),
                        });
                        self.placeholder_ir()
                    }
                } else {
                    self.errors.push(SemanticError::ArgsCountMismatch { span });
                    self.placeholder_ir()
                }
            }
            TypeInfo::Type(type_id) => self.handle_type_cast(span, &expr.args, type_id),
            TypeInfo::Int => self.handle_type_cast(span, &expr.args, self.predefined_types.t_int),
            _ => {
                self.errors.push(SemanticError::NotCallable {
                    callee: expr.callee.as_ref().clone(),
                });
                self.placeholder_ir()
            }
        }
    }

    fn handle_binary_expr(&mut self, span: Span, expr: &BinaryExpr) -> Ir {
        let lhs = self.handle_expr(&expr.lhs);
        let rhs = self.handle_expr(&expr.rhs);

        if lhs.ty == self.predefined_types.t_string {
            if expr.op == BinaryOp::Add {
                if rhs.ty == lhs.ty {
                    let mut irs = vec![];
                    flatten_add_ir(lhs, &mut irs);
                    flatten_add_ir(rhs, &mut irs);
                    return Ir {
                        ty: self.predefined_types.t_string,
                        kind: ir::ExprKind::Concat(irs),
                    };
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        span,
                        expect: lhs.ty,
                        found: rhs.ty,
                    });
                    return self.placeholder_ir();
                }
            } else {
                self.errors.push(SemanticError::InvalidBinaryOp {
                    span: expr.op_span.clone(),
                    op: expr.op,
                });
                return self.placeholder_ir();
            }
        }

        let type_id = if lhs.ty == rhs.ty {
            lhs.ty
        } else {
            self.emit_type_mismatch_error(span.clone(), lhs.ty, rhs.ty);
            self.predefined_types.t_any
        };

        let kind = match expr.op {
            BinaryOp::Add => ir::ExprKind::Add((lhs.into(), rhs.into())),
            BinaryOp::Sub => ir::ExprKind::Sub((lhs.into(), rhs.into())),
            BinaryOp::Mul => ir::ExprKind::Mul((lhs.into(), rhs.into())),
            BinaryOp::Div => ir::ExprKind::Div((lhs.into(), rhs.into())),
        };

        Ir { kind, ty: type_id }
    }

    fn handle_unary_expr(&mut self, expr: &UnaryExpr) -> Ir {
        let ir = self.handle_expr(&expr.expr);
        match expr.op {
            UnaryOp::Plus => ir,
            UnaryOp::Minus => {
                let expected_types = vec![
                    self.predefined_types.t_int,
                    self.predefined_types.t_float,
                    self.predefined_types.t_rational,
                ];
                if expected_types.contains(&ir.ty) {
                    Ir {
                        ty: ir.ty,
                        kind: ir::ExprKind::Neg(ir.into()),
                    }
                } else {
                    self.errors.push(SemanticError::TypeError {
                        span: expr.expr.span.clone(),
                        kind: TypeError::InvalidUnaryOperand {
                            op: expr.op,
                            operand: ir.ty,
                            expected: expected_types,
                        },
                    });
                    self.placeholder_ir()
                }
            }
            UnaryOp::Not => Ir {
                ty: self.predefined_types.t_logic,
                kind: ir::ExprKind::Not(ir.into()),
            },
        }
    }

    fn handle_type_expr(&mut self, expr: &TypeExpr) -> TypeId {
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
                            return self.predefined_types.t_any;
                        }
                    }
                }
                self.errors.push(SemanticError::TypeNotFound {
                    span: expr.span.clone(),
                    symbol: *symbol,
                });
                self.predefined_types.t_any
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
            TypeExprKind::Array(elem_type) => {
                let elem_type_id = self.handle_type_expr(elem_type);
                self.types.intern(TypeInfo::Array(elem_type_id))
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

    fn handle_member_expr(&mut self, expr: &MemberExpr) -> Ir {
        let obj = self.handle_expr(&expr.object);

        if obj.ty == self.predefined_types.t_string {
            if let ExprKind::Id(id_expr) = &expr.property.kind {
                if id_expr.symbol == self.builtin_symbols.s_Length {
                    return Ir {
                        kind: ir::ExprKind::GetLength(obj.into()),
                        ty: self.predefined_types.t_int,
                    };
                }
            }
        }

        self.placeholder_ir()
    }

    fn handle_construct_expr(&mut self, cons_expr: &ConstructExpr) -> Ir {
        if let ExprKind::Id(id_expr) = &cons_expr.callee.kind {
            if id_expr.symbol == self.builtin_symbols.s_option {
                return self.handle_construct_option(cons_expr);
            }
            if id_expr.symbol == self.builtin_symbols.s_array {
                return self.handle_construct_array(cons_expr);
            }
        }

        todo!()
    }

    fn handle_construct_option(&mut self, cons_expr: &ConstructExpr) -> Ir {
        if let Some(value) = cons_expr.args.first() {
            let value = self.handle_expr(value);
            let ty = self.types.intern(TypeInfo::Option(value.ty));
            Ir {
                kind: ir::ExprKind::Option(Some(value.into())),
                ty,
            }
        } else {
            self.placeholder_ir()
        }
    }

    fn handle_construct_array(&mut self, cons_expr: &ConstructExpr) -> Ir {
        if cons_expr.args.is_empty() {
            todo!();
        }

        let mut irs: Vec<Ir> = vec![];
        for (i, arg) in cons_expr.args.iter().enumerate() {
            let ir = self.handle_expr(arg);
            if i > 0 {
                let prev = irs.last().unwrap();
                if ir.ty != prev.ty {
                    self.errors.push(SemanticError::TypeMismatch {
                        span: arg.span.clone(),
                        expect: prev.ty,
                        found: ir.ty,
                    });
                    return self.placeholder_ir();
                }
            }
            irs.push(ir);
        }

        Ir {
            ty: self.types.intern(TypeInfo::Array(irs.first().unwrap().ty)),
            kind: ir::ExprKind::Array(irs),
        }
    }
}

fn flatten_add_ir(ir: Ir, out: &mut Vec<Ir>) {
    if let ir::ExprKind::Add((lhs, rhs)) = ir.kind {
        flatten_add_ir(*lhs, out);
        flatten_add_ir(*rhs, out);
    } else {
        out.push(ir);
    }
}

#[derive(Debug)]
pub enum TypeError {
    InvalidUnaryOperand {
        op: UnaryOp,
        operand: TypeId,
        expected: Vec<TypeId>,
    },
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

    #[error("is not callable")]
    NotCallable { callee: Expression },

    #[error("type error")]
    TypeError { span: Span, kind: TypeError },

    #[error("'break' is not allowed outside of a loop")]
    BreakOutsideLoop { span: Span },

    #[error("cannot apply {:?}", op)]
    InvalidBinaryOp { span: Span, op: BinaryOp },
}
