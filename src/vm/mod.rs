use derive_more::Constructor;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    compiler::ir::UpvalueDesc,
    core::{ConstValue, types::PredefinedTypes},
    runtime::{
        CallContext, FnKind, FunctionId, TypeId, Value,
        heap::{Heap, ObjectId, SimpleHeap},
    },
};

pub mod global_vars;

#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Opcode {
    PushVoid,
    PushInt,
    PushFloat,
    PushChar,
    PushChar32,
    PushString,
    PushTrue,
    PushFalse,
    PushNone,
    PushType,

    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Not,

    Dup,
    Pop,

    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,

    Jmp,

    StoreLocal,
    LoadLocal,
    StoreGlobal,
    LoadGlobal,
    StoreUpvalue,
    LoadUpvalue,

    MakeOption,
    MakeTuple,
    MakeClosure,
    MakeArray,

    IndexTuple,

    ToString,
    Concat,

    Call,
    Cast,
    Len,
}

#[derive(Clone, Copy)]
pub struct FailureHandler {
    /// First protected instruction (inclusive)
    pub start_pc: u32,

    /// End of protected region (exclusive)
    pub end_pc: u32,

    /// First instruction of the catch block
    pub handler_pc: u32,

    /// Operand stack size to restore
    pub op_stack_size: u16,
}

#[derive(Constructor, Clone)]
pub struct Function {
    pub type_id: TypeId,
    pub bytecode: Vec<u8>,
    pub failure_table: Vec<FailureHandler>,
    pub upvalues: Vec<UpvalueDesc>,
}

struct Frame {
    func_id: usize,
    stack_base: usize,
    pc: usize,
    upvalues: Vec<ObjectId>,
}

pub struct Vm<H: Heap = SimpleHeap> {
    op_stack: Vec<Value>,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    heap: H,
    const_table: Vec<ConstValue>,
    predefined_types: PredefinedTypes,
    pub functions: Vec<Function>,

    has_pending_failure: bool,
}

impl Vm {
    pub fn new(const_table: Vec<ConstValue>, predefined_types: PredefinedTypes) -> Self {
        Self {
            op_stack: vec![],
            stack: vec![],
            frames: vec![],
            heap: SimpleHeap::default(),
            const_table,
            predefined_types,
            functions: vec![],
            has_pending_failure: false,
        }
    }

    pub fn run(&mut self, func_id: usize) {
        self.frames.push(Frame {
            func_id,
            stack_base: 0,
            pc: 0,
            upvalues: vec![],
        });

        loop {
            let frame = match self.frames.last_mut() {
                Some(frame) => frame,
                None => break,
            };

            let func = &self.functions[frame.func_id];

            let op = match func.bytecode.get(frame.pc) {
                Some(byte) => {
                    frame.pc += 1;
                    Opcode::try_from(*byte).unwrap()
                }
                _ => {
                    self.stack.truncate(frame.stack_base);
                    self.frames.pop();
                    continue;
                }
            };

            self.dispatch(op);

            if self.has_pending_failure {
                while let Some(frame) = self.frames.last_mut() {
                    let func = &self.functions[frame.func_id];
                    let handler = func.failure_table.iter().find(|ft| {
                        frame.pc >= ft.start_pc as usize && frame.pc < ft.end_pc as usize
                    });
                    if let Some(handler) = handler {
                        self.has_pending_failure = false;
                        frame.pc = handler.handler_pc as usize;
                        self.op_stack.truncate(handler.op_stack_size as usize);
                        break;
                    }
                    self.frames.pop();
                }

                if self.has_pending_failure {
                    self.op_stack.clear();
                    break;
                }
            }
        }

        assert_eq!(self.op_stack.pop(), None);
    }

    fn get_stack_index(&self, offset: usize) -> usize {
        self.frames.last().unwrap().stack_base + offset
    }

    fn read_byte(&mut self) -> u8 {
        let frame = self.frames.last_mut().unwrap();
        let func = &self.functions[frame.func_id];
        let byte = func.bytecode[frame.pc];
        frame.pc += 1;
        byte
    }

    fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes([
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
        ])
    }

    fn read_i64(&mut self) -> i64 {
        i64::from_le_bytes([
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
        ])
    }

    fn read_f64(&mut self) -> f64 {
        f64::from_le_bytes([
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
            self.read_byte(),
        ])
    }

    fn dispatch(&mut self, op: Opcode) {
        match op {
            Opcode::PushVoid => self.exec_push_void(),
            Opcode::PushInt => self.exec_push_int(),
            Opcode::PushFloat => self.exec_push_float(),
            Opcode::PushChar => self.exec_push_char(),
            Opcode::PushChar32 => self.exec_push_char32(),
            Opcode::PushString => self.exec_push_string(),
            Opcode::PushTrue => self.exec_push_logic(true),
            Opcode::PushFalse => self.exec_push_logic(false),
            Opcode::PushNone => self.exec_push_none(),
            Opcode::PushType => self.exec_push_type(),
            Opcode::StoreLocal => self.exec_store_local(),
            Opcode::LoadLocal => self.exec_load_local(),
            Opcode::StoreGlobal => self.exec_store_global(),
            Opcode::LoadGlobal => self.exec_load_global(),
            Opcode::StoreUpvalue => self.exec_store_upvalue(),
            Opcode::LoadUpvalue => self.exec_load_upvalue(),
            Opcode::MakeOption => self.exec_make_option(),
            Opcode::MakeTuple => self.exec_make_tuple(),
            Opcode::MakeArray => self.exec_make_array(),
            Opcode::MakeClosure => self.exec_make_closure(),
            Opcode::IndexTuple => self.exec_index_tuple(),
            Opcode::Add => self.exec_add(),
            Opcode::Sub => self.exec_sub(),
            Opcode::Mul => self.exec_mul(),
            Opcode::Div => self.exec_div(),
            Opcode::Neg => self.exec_neg(),
            Opcode::Not => self.exec_not(),
            Opcode::Dup => self.exec_dup(),
            Opcode::Pop => self.exec_pop(),
            Opcode::Eq => self.exec_eq(),
            Opcode::Ne => self.exec_ne(),
            Opcode::Gt => self.exec_gt(),
            Opcode::Ge => self.exec_ge(),
            Opcode::Lt => self.exec_lt(),
            Opcode::Le => self.exec_le(),
            Opcode::Jmp => self.exec_jmp(),
            Opcode::Call => self.exec_call(),
            Opcode::ToString => self.exec_to_string(),
            Opcode::Concat => self.exec_concat(),
            Opcode::Cast => self.exec_cast(),
            Opcode::Len => self.exec_len(),
        }
    }

    fn set_stack_value(&mut self, index: usize, value: Value) {
        if index < self.stack.len() {
            self.stack[index] = value;
        } else if index == self.stack.len() {
            self.stack.push(value);
        } else {
            panic!("stack slot must be allocated continuously")
        }
    }

    fn box_local(&mut self, offset: usize) -> ObjectId {
        let index = self.get_stack_index(offset);
        if let Value::Ref(obj_id) = &self.stack[index] {
            return *obj_id;
        }
        let value = std::mem::take(&mut self.stack[index]);
        let obj_id = self.heap.alloc_obj(value);
        self.stack[index] = Value::Ref(obj_id);
        obj_id
    }

    fn resolve_ref<'a>(&'a self, mut value: &'a Value) -> &'a Value {
        while let Value::Ref(id) = value {
            value = self.heap.fetch_obj(*id);
        }
        value
    }

    fn exec_push_void(&mut self) {
        self.op_stack.push(Value::Void);
    }

    fn exec_push_int(&mut self) {
        let value = Value::Integer(self.read_i64());
        self.op_stack.push(value);
    }

    fn exec_push_float(&mut self) {
        let value = Value::Float(self.read_f64());
        self.op_stack.push(value);
    }

    fn exec_push_char(&mut self) {
        let value = Value::Char(self.read_byte());
        self.op_stack.push(value);
    }

    fn exec_push_char32(&mut self) {
        let ch = unsafe { char::from_u32_unchecked(self.read_u32()) };
        self.op_stack.push(Value::Char32(ch));
    }

    fn exec_push_string(&mut self) {
        let index = self.read_u32() as usize;
        let str = match &self.const_table[index] {
            ConstValue::String(s) => s.to_owned(),
        };
        let val = Value::String(str);
        let obj_id = self.heap.alloc_obj(val);
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_push_logic(&mut self, value: bool) {
        self.op_stack.push(Value::Logic(value));
    }

    fn exec_push_none(&mut self) {
        let type_id = TypeId(self.read_u32());
        self.op_stack.push(Value::Option {
            type_id,
            value: None,
        });
    }

    fn exec_push_type(&mut self) {
        let type_id = self.read_u32();
        self.op_stack.push(Value::Type(TypeId(type_id)));
    }

    fn exec_store_local(&mut self) {
        let value = self.op_stack.last().unwrap().clone();
        let base = self.frames.last().unwrap().stack_base;
        let offset = self.read_u32() as usize;
        let index = base + offset;
        self.set_stack_value(index, value);
    }

    fn exec_load_local(&mut self) {
        let base = self.frames.last().unwrap().stack_base;
        let offset = self.read_u32() as usize;
        let index = base + offset;
        let value = self.stack[index].clone();
        self.op_stack.push(value);
    }

    fn exec_store_global(&mut self) {
        let value = self.op_stack.last().unwrap().clone();
        let index = self.read_u32() as usize;
        self.set_stack_value(index, value);
    }

    fn exec_load_global(&mut self) {
        let index = self.read_u32() as usize;
        let value = self.stack[index].clone();
        self.op_stack.push(value);
    }

    fn exec_store_upvalue(&mut self) {
        let value = self.op_stack.last().unwrap().clone();
        let index = self.read_u32() as usize;
        let obj_id = self.frames.last().unwrap().upvalues[index];
        self.heap.update_obj(obj_id, value);
    }

    fn exec_load_upvalue(&mut self) {
        let index = self.read_u32() as usize;
        let obj_id = self.frames.last().unwrap().upvalues[index];
        let value = self.heap.fetch_obj(obj_id).clone();
        self.op_stack.push(value);
    }

    fn exec_make_option(&mut self) {
        let type_id = TypeId(self.read_u32());
        let val = self.op_stack.pop().unwrap();
        self.op_stack.push(Value::Option {
            type_id,
            value: Some(val.into()),
        });
    }

    fn exec_make_tuple(&mut self) {
        let type_id = TypeId(self.read_u32());
        let elem_cnt = self.read_u32();
        let start = self.op_stack.len() - elem_cnt as usize;
        let elements = self.op_stack.split_off(start);
        let value = Value::Tuple { type_id, elements };
        self.op_stack.push(value);
    }

    fn exec_make_array(&mut self) {
        let type_id = self.read_u32();
        let elem_cnt = self.read_u32();
        let start = self.op_stack.len() - elem_cnt as usize;
        let elements = self.op_stack.split_off(start);
        let value = Value::Array {
            type_id: TypeId(type_id),
            elements,
        };
        self.op_stack.push(value);
    }

    fn exec_index_tuple(&mut self) {
        let elem_idx = self.read_byte() as usize;
        let elems = match self.op_stack.pop().unwrap() {
            Value::Tuple { elements, .. } => elements,
            _ => unreachable!(),
        };
        self.op_stack.push(elems[elem_idx].clone());
    }

    fn exec_add(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        self.op_stack.push(lhs + rhs);
    }

    fn exec_sub(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        self.op_stack.push(lhs - rhs);
    }

    fn exec_mul(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        self.op_stack.push(lhs * rhs);
    }

    fn exec_div(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if rhs.is_zero() {
            self.has_pending_failure = true;
            return;
        }
        self.op_stack.push(lhs / rhs);
    }

    fn exec_neg(&mut self) {
        let value = self.op_stack.pop().unwrap();
        self.op_stack.push(-value);
    }

    fn exec_not(&mut self) {
        let value = self.op_stack.pop().unwrap();
        self.op_stack.push(!value);
    }

    fn exec_dup(&mut self) {
        let value = self.op_stack.last().unwrap().clone();
        self.op_stack.push(value);
    }

    fn exec_pop(&mut self) {
        self.op_stack.pop();
    }

    fn exec_eq(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs == rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_ne(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs != rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_gt(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs > rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_ge(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs >= rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_lt(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs < rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_le(&mut self) {
        let rhs = self.op_stack.pop().unwrap();
        let lhs = self.op_stack.pop().unwrap();
        if lhs <= rhs {
            self.op_stack.push(rhs);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_jmp(&mut self) {
        let pc = self.read_u32() as usize;
        self.frames.last_mut().unwrap().pc = pc;
    }

    fn exec_make_closure(&mut self) {
        let func_id = self.read_u32() as usize;
        let func = &self.functions[func_id];
        let func_type = func.type_id;
        let upvalues = func.upvalues.clone();

        let upvalues = upvalues
            .into_iter()
            .map(|upvalue| match upvalue {
                UpvalueDesc::Local(slot) => self.box_local(slot.0),
                UpvalueDesc::Upvalue(index) => {
                    let parent_frame = self.frames.iter().rev().next().unwrap();
                    parent_frame.upvalues[index]
                }
            })
            .collect();

        let func_val = Value::Function {
            type_id: func_type,
            kind: FnKind::Verse {
                id: FunctionId(func_id),
                upvalues,
            },
        };
        let obj_id = self.heap.alloc_obj(func_val);
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_call(&mut self) {
        let argc = self.read_u32();
        let args = self.op_stack.split_off(self.op_stack.len() - argc as usize);
        let func = match self.op_stack.pop().unwrap() {
            Value::Ref(obj_id) => match self.heap.fetch_obj(obj_id) {
                Value::Function { kind, .. } => kind,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        match func {
            FnKind::Verse { id, upvalues } => {
                let frame = Frame {
                    func_id: id.0,
                    stack_base: self.stack.len(),
                    pc: 0,
                    upvalues: upvalues.clone(),
                };
                for arg in args {
                    self.stack.push(arg);
                }
                self.frames.push(frame);
            }
            FnKind::Native(native_fn) => {
                let mut ctx = CallContext {
                    heap: &self.heap,
                    args: &args,
                    ret_val: None,
                };
                native_fn(&mut ctx);
                if let Some(ret_val) = ctx.ret_val {
                    if let Ok(ret_val) = ret_val {
                        self.op_stack.push(ret_val);
                    } else {
                        self.has_pending_failure = true;
                    }
                } else {
                    self.op_stack.push(Value::Void);
                }
            }
        };
    }

    fn exec_to_string(&mut self) {
        let value = self.op_stack.pop().unwrap();
        let value = match value {
            Value::String(_) | Value::Ref(_) => value,
            _ => Value::String(value.to_string()),
        };
        self.op_stack.push(value);
    }

    fn exec_concat(&mut self) {
        let count = self.read_u32() as usize;
        let values = self.op_stack.split_off(self.op_stack.len() - count);
        let mut buf = String::new();
        for value in values {
            let value = self.resolve_ref(&value);
            match value {
                Value::String(s) => buf.push_str(s),
                _ => panic!("not string"),
            }
        }
        let obj_id = self.heap.alloc_obj(Value::String(buf));
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_cast(&mut self) {
        let expect = TypeId(self.read_u32());
        let value = self.op_stack.last().unwrap();
        let value = self.resolve_ref(value);
        let ok = match value {
            Value::Void => expect == self.predefined_types.t_void,
            Value::Integer(_) => expect == self.predefined_types.t_int,
            Value::Rational(..) => expect == self.predefined_types.t_rational,
            Value::Float(_) => expect == self.predefined_types.t_float,
            Value::Char(_) => expect == self.predefined_types.t_char,
            Value::Char32(_) => expect == self.predefined_types.t_char32,
            Value::String(_) => expect == self.predefined_types.t_string,
            Value::False => expect == self.predefined_types.t_false,
            Value::Logic(_) => expect == self.predefined_types.t_logic,
            Value::Option { type_id, .. } => expect == *type_id,
            Value::Tuple { type_id, .. } => expect == *type_id,
            Value::Array { type_id, .. } => expect == *type_id,
            Value::Function { type_id, .. } => expect == *type_id,
            Value::Type(type_id) => expect == *type_id,
            Value::Ref(_) => unreachable!(),
        };
        if !ok {
            self.has_pending_failure = true;
        }
    }

    fn exec_len(&mut self) {
        let value = self.op_stack.pop().unwrap();
        let value = self.resolve_ref(&value);
        let len = match value {
            Value::String(s) => s.len(),
            Value::Array { elements, .. } => elements.len(),
            _ => unreachable!(),
        };
        self.op_stack.push(Value::Integer(len as i64));
    }
}
