use crate::{
    core::{
        ConstId,
        types::{PredefinedTypes, TypeInfo, TypeRegistry},
    },
    runtime::TypeId,
    vm::{FailureHandler, Function, Opcode},
};

use super::ast::CompareOp;
use super::ir::{ExprKind, FunctionExpr, IfExpr, Ir, Slot, TemplateElement};

#[derive(Default)]
struct LoopContext {
    break_jmp_target_indices: Vec<u32>,
}

pub struct Compiler {
    pub bytecode: Vec<u8>,
    pub failure_handlers: Vec<FailureHandler>,
    op_stack_size: u16,
    loop_ctx_stack: Vec<LoopContext>,
    functions: Vec<Function>,
    start_fn_id: usize,
    pub type_registry: TypeRegistry,
    pub predefined_types: PredefinedTypes,
}

impl Compiler {
    pub fn new() -> Self {
        let mut type_reg = TypeRegistry::new();
        let predefined_types = PredefinedTypes::install(&mut type_reg);
        Self {
            type_registry: type_reg,
            predefined_types,
            bytecode: vec![],
            failure_handlers: vec![],
            op_stack_size: 0,
            loop_ctx_stack: vec![],
            functions: vec![],
            start_fn_id: 0,
        }
    }

    pub fn compile(&mut self, irs: Vec<Ir>) -> Vec<Function> {
        for ir in irs {
            self.compile_ir(ir);
            self.append_op(Opcode::Pop, -1);
        }
        let func = Function {
            type_id: self.intern_type(TypeInfo::Any),
            bytecode: self.bytecode.clone(),
            failure_table: self.failure_handlers.clone(),
            upvalues: vec![],
        };
        self.functions.push(func);
        self.functions.clone()
    }

    pub fn compile_ir(&mut self, ir: Ir) {
        match ir.kind {
            ExprKind::Int(v) => self.compile_int(v),
            ExprKind::Float(v) => self.compile_float(v),
            ExprKind::Char(v) => self.compile_char(v),
            ExprKind::Char32(v) => self.compile_char32(v),
            ExprKind::String(id) => self.compile_string(id),
            ExprKind::Logic(v) => self.compile_logic(v),
            ExprKind::Option(v) => self.compile_option(ir.ty, v),
            ExprKind::StoreLocal { slot, value } => self.compile_store_local(slot, *value),
            ExprKind::LoadLocal { slot } => self.compile_load_local(slot),
            ExprKind::StoreGlobal { slot, value } => self.compile_store_global(slot, *value),
            ExprKind::LoadGlobal { slot } => self.compile_load_global(slot),
            ExprKind::StoreUpvalue { index, value } => self.compile_store_upvalue(index, *value),
            ExprKind::LoadUpvalue { index } => self.compile_load_upvalue(index),
            ExprKind::Tuple(elems) => self.compile_make(Opcode::MakeTuple, ir.ty, elems),
            ExprKind::Array(elems) => self.compile_make(Opcode::MakeArray, ir.ty, elems),
            ExprKind::IndexTuple { tuple, index } => self.compile_index_tuple(*tuple, index),
            ExprKind::Add((lhs, rhs)) => self.compile_bin_op(*lhs, *rhs, Opcode::Add),
            ExprKind::Sub((lhs, rhs)) => self.compile_bin_op(*lhs, *rhs, Opcode::Sub),
            ExprKind::Mul((lhs, rhs)) => self.compile_bin_op(*lhs, *rhs, Opcode::Mul),
            ExprKind::Div((lhs, rhs)) => self.compile_bin_op(*lhs, *rhs, Opcode::Div),
            ExprKind::Neg(v) => self.compile_unary_op(*v, Opcode::Neg),
            ExprKind::Not(v) => self.compile_unary_op(*v, Opcode::Not),
            ExprKind::If(e) => self.compile_if(ir.ty, e),
            ExprKind::CompareChain(e) => self.compile_cmp_chain(*e.head, e.rest),
            ExprKind::Loop(ir) => self.compile_loop(*ir),
            ExprKind::Break => self.compile_break(),
            ExprKind::Block(irs) => self.compile_block(irs),
            ExprKind::Func(fn_ir) => self.compile_function(fn_ir, ir.ty),
            ExprKind::Call(ir) => self.compile_call(*ir.callee, ir.args),
            ExprKind::Template(elems) => self.compile_template(elems),
            ExprKind::Type(type_id) => self.compile_type_literal(type_id),
            ExprKind::Cast { ty, value } => self.compile_cast(ty, *value),
            ExprKind::GetLength(ir) => self.compile_get_len(*ir),
            ExprKind::Concat(irs) => self.compile_concat(irs),
        }
    }

    fn append_u8(&mut self, value: u8) {
        self.bytecode.push(value);
    }

    fn write_u8(&mut self, index: usize, value: u8) {
        if index == self.bytecode.len() {
            self.append_u8(value);
        } else {
            self.bytecode[index] = value;
        }
    }

    fn append_bytes(&mut self, value: &[u8]) {
        for byte in value {
            self.append_u8(*byte);
        }
    }

    fn write_bytes(&mut self, index: usize, value: &[u8]) {
        for (offset, byte) in value.iter().enumerate() {
            self.write_u8(index + offset, *byte);
        }
    }

    fn append_u32(&mut self, value: u32) {
        self.append_bytes(&value.to_le_bytes());
    }

    fn write_u32(&mut self, index: usize, value: u32) {
        self.write_bytes(index, &value.to_le_bytes());
    }

    fn append_op(&mut self, op: Opcode, op_stack_size_change: i16) {
        self.append_u8(op.into());
        self.op_stack_size = (self.op_stack_size as i16 + op_stack_size_change) as u16
    }

    fn intern_type(&mut self, type_info: TypeInfo) -> TypeId {
        self.type_registry.intern(type_info)
    }

    fn compile_int(&mut self, value: i64) {
        self.append_op(Opcode::PushInt, 1);
        self.append_bytes(&value.to_le_bytes());
    }

    fn compile_float(&mut self, value: f64) {
        self.append_op(Opcode::PushFloat, 1);
        self.append_bytes(&value.to_le_bytes());
    }

    fn compile_char(&mut self, value: u8) {
        self.append_op(Opcode::PushChar, 1);
        self.append_u8(value);
    }

    fn compile_char32(&mut self, value: char) {
        self.append_op(Opcode::PushChar32, 1);
        self.append_bytes(&(value as u32).to_le_bytes());
    }

    fn compile_string(&mut self, id: ConstId) {
        self.append_op(Opcode::PushString, 1);
        self.append_u32(id.0 as u32);
    }

    fn compile_logic(&mut self, value: bool) {
        self.append_op(
            if value {
                Opcode::PushTrue
            } else {
                Opcode::PushFalse
            },
            1,
        );
    }

    fn compile_make_option(&mut self, option_type: TypeInfo) {
        let type_id = self.intern_type(option_type);
        self.append_op(Opcode::MakeOption, 0);
        self.append_u32(type_id.0);
    }

    fn compile_push_none(&mut self, option_type: TypeInfo) {
        let type_id = self.intern_type(option_type);
        self.append_op(Opcode::PushNone, 1);
        self.append_u32(type_id.0);
    }

    fn compile_option(&mut self, type_info: TypeInfo, value: Option<Box<Ir>>) {
        if let Some(value) = value {
            self.compile_ir(*value);
            self.compile_make_option(type_info);
        } else {
            self.compile_push_none(type_info);
        }
    }

    fn compile_store_local(&mut self, slot: Slot, value: Ir) {
        self.compile_ir(value);
        self.append_op(Opcode::StoreLocal, 0);
        self.append_u32(slot.0 as u32);
    }

    fn compile_load_local(&mut self, slot: Slot) {
        self.append_op(Opcode::LoadLocal, 1);
        self.append_u32(slot.0 as u32);
    }

    fn compile_store_global(&mut self, slot: Slot, value: Ir) {
        self.compile_ir(value);
        self.append_op(Opcode::StoreGlobal, 0);
        self.append_u32(slot.0 as u32);
    }

    fn compile_load_global(&mut self, slot: Slot) {
        self.append_op(Opcode::LoadGlobal, 1);
        self.append_u32(slot.0 as u32);
    }

    fn compile_store_upvalue(&mut self, index: usize, value: Ir) {
        self.compile_ir(value);
        self.append_op(Opcode::StoreUpvalue, 0);
        self.append_u32(index as u32);
    }

    fn compile_load_upvalue(&mut self, index: usize) {
        self.append_op(Opcode::LoadUpvalue, 1);
        self.append_u32(index as u32);
    }

    fn compile_make(&mut self, op: Opcode, type_info: TypeInfo, irs: Vec<Ir>) {
        let type_id = self.intern_type(type_info);
        let argc = irs.len();
        assert!(argc < u16::MAX as usize);
        for ir in irs {
            self.compile_ir(ir);
        }
        self.append_op(op, -(argc as i16) + 1);
        self.append_u32(type_id.0 as u32);
        self.append_u32(argc as u32);
    }

    fn compile_index_tuple(&mut self, value: Ir, index: usize) {
        assert!(index < u8::MAX as usize);
        self.compile_ir(value);
        self.append_op(Opcode::IndexTuple, 0);
        self.append_u8(index as u8);
    }

    fn compile_bin_op(&mut self, lhs: Ir, rhs: Ir, op: Opcode) {
        self.compile_ir(lhs);
        self.compile_ir(rhs);
        self.append_op(op, -1);
    }

    fn compile_unary_op(&mut self, value: Ir, op: Opcode) {
        self.compile_ir(value);
        self.append_op(op, 0);
    }

    fn compile_cmp_chain(&mut self, head: Ir, rest: Vec<(CompareOp, Ir)>) {
        self.compile_ir(head);
        self.append_op(Opcode::Dup, 1);
        for (op, ir) in rest {
            self.compile_ir(ir);
            self.append_op(
                match op {
                    CompareOp::Eq => Opcode::Eq,
                    CompareOp::Ne => Opcode::Ne,
                    CompareOp::Gt => Opcode::Gt,
                    CompareOp::Ge => Opcode::Ge,
                    CompareOp::Lt => Opcode::Lt,
                    CompareOp::Le => Opcode::Le,
                },
                -1,
            );
        }
        self.append_op(Opcode::Pop, -1);
    }

    fn compile_if(&mut self, type_info: TypeInfo, if_ir: IfExpr) {
        let mut handler = FailureHandler {
            start_pc: self.bytecode.len() as u32,
            end_pc: 0,
            handler_pc: 0,
            op_stack_size: self.op_stack_size,
        };

        self.compile_ir(*if_ir.test);
        self.append_op(Opcode::Pop, -1); // pop test result
        handler.end_pc = self.bytecode.len() as u32;

        self.compile_ir(*if_ir.then);
        if if_ir.alt.is_none() {
            self.compile_make_option(type_info.clone());
        }
        self.append_op(Opcode::Jmp, 0);
        let jmp_target = self.bytecode.len();
        self.append_u32(0);

        handler.handler_pc = self.bytecode.len() as u32;
        if let Some(alt) = if_ir.alt {
            self.compile_ir(*alt);
        } else {
            self.compile_push_none(type_info);
        }

        self.write_u32(jmp_target, self.bytecode.len() as u32);
        self.failure_handlers.push(handler);
    }

    fn compile_loop(&mut self, ir: Ir) {
        self.loop_ctx_stack.push(LoopContext::default());

        let loop_start = self.bytecode.len();
        self.compile_ir(ir);
        self.append_op(Opcode::Jmp, 0);
        self.append_u32(loop_start as u32);
        let loop_end = self.bytecode.len();

        let loop_ctx = self.loop_ctx_stack.pop().unwrap();
        for index in loop_ctx.break_jmp_target_indices {
            self.write_u32(index as usize, loop_end as u32);
        }
    }

    fn compile_break(&mut self) {
        self.append_op(Opcode::Jmp, 0);
        let jmp_target_index = self.bytecode.len();
        self.append_u32(u32::MAX);
        self.loop_ctx_stack
            .last_mut()
            .unwrap()
            .break_jmp_target_indices
            .push(jmp_target_index as u32);
    }

    fn compile_block(&mut self, irs: Vec<Ir>) {
        for (i, ir) in irs.into_iter().enumerate() {
            if i > 0 {
                self.append_op(Opcode::Pop, -1);
            }
            self.compile_ir(ir);
        }
    }

    fn compile_function(&mut self, fn_ir: FunctionExpr, fn_type: TypeInfo) {
        let mut compiler = Compiler::new();
        compiler.start_fn_id = self.functions.len();
        compiler.compile_function_body(*fn_ir.body, fn_ir.return_void);
        let func = Function {
            type_id: self.intern_type(fn_type),
            bytecode: compiler.bytecode,
            failure_table: compiler.failure_handlers,
            upvalues: fn_ir.upvalues,
        };
        self.functions.extend(compiler.functions);
        let fn_id = self.start_fn_id + self.functions.len();
        self.functions.push(func);

        self.append_op(Opcode::MakeClosure, 1);
        self.append_u32(fn_id as u32);
        self.append_op(Opcode::StoreLocal, 0);
        self.append_u32(fn_ir.slot.0 as u32);
    }

    fn compile_function_body(&mut self, body: Ir, return_void: bool) {
        if let ExprKind::Block(irs) = body.kind {
            let len = irs.len();
            for (i, ir) in irs.into_iter().enumerate() {
                self.compile_ir(ir);
                let is_last = i == len - 1;
                if !is_last || return_void {
                    self.append_op(Opcode::Pop, -1);
                }
            }
            if return_void {
                self.append_op(Opcode::PushVoid, 1);
            }
        } else {
            self.compile_ir(body);
            if return_void {
                self.append_op(Opcode::Pop, -1);
                self.append_op(Opcode::PushVoid, 1);
            }
        }
    }

    fn compile_call(&mut self, callee: Ir, args: Vec<Ir>) {
        self.compile_ir(callee);
        let argc = args.len();
        for arg in args {
            self.compile_ir(arg);
        }
        self.append_op(Opcode::Call, -(argc as i16));
        self.append_u32(argc as u32);
    }

    fn compile_concat(&mut self, irs: Vec<Ir>) {
        let count = irs.len();
        for ir in irs {
            self.compile_ir(ir);
        }
        self.append_op(Opcode::Concat, -(count as i16) + 1);
        self.append_u32(count as u32);
    }

    fn compile_template(&mut self, elements: Vec<TemplateElement>) {
        let count = elements.len();
        for elem in elements {
            match elem {
                TemplateElement::Expr(ir) => {
                    self.compile_ir(*ir);
                    self.append_op(Opcode::ToString, 0);
                }
                TemplateElement::String(id) => {
                    self.append_op(Opcode::PushString, 1);
                    self.append_u32(id.0 as u32);
                    self.op_stack_size += 1;
                }
            }
        }
        self.append_op(Opcode::Concat, -(count as i16) + 1);
        self.append_u32(count as u32);
    }

    fn compile_type_literal(&mut self, type_info: TypeInfo) {
        let type_id = self.intern_type(type_info);
        self.append_op(Opcode::PushType, 1);
        self.append_u32(type_id.0 as u32);
    }

    fn compile_cast(&mut self, type_info: TypeInfo, value: Ir) {
        let type_id = self.intern_type(type_info);
        self.compile_ir(value);
        self.append_op(Opcode::Cast, 0);
        self.append_u32(type_id.0 as u32);
    }

    fn compile_get_len(&mut self, value: Ir) {
        self.compile_ir(value);
        self.append_op(Opcode::Len, 0);
    }
}
