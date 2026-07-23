use std::rc::Rc;

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
    PushMethod,

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
    MakeObject,
    LoadObjectField,
    StoreObjectField,

    LoadTupleElement,
    StoreTupleElement,
    LoadArrayElement,
    StoreArrayElement,

    ToString,
    Concat,

    Call,
    Cast,
    Len,
    Unwrap,
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

pub struct Class {
    pub methods: Vec<FunctionId>,
}

pub struct Vm<H: Heap = SimpleHeap> {
    op_stack: Vec<Value>,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    heap: H,
    const_table: Vec<ConstValue>,
    predefined_types: PredefinedTypes,
    pub functions: Vec<Function>,
    pub classes: Vec<Class>,

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
            classes: vec![],
            has_pending_failure: false,
        }
    }

    pub fn run(&mut self, func_id: usize) {
        self.frames.push(Frame {
            func_id,
            stack_base: self.stack.len(),
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

        assert_eq!(self.op_stack.len(), 0);
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
            Opcode::PushMethod => self.exec_push_method(),
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
            Opcode::MakeObject => self.exec_make_object(),
            Opcode::LoadTupleElement => self.exec_index_tuple(),
            Opcode::StoreTupleElement => self.exec_set_tuple_element(),
            Opcode::LoadArrayElement => self.exec_index_array(),
            Opcode::StoreArrayElement => self.exec_set_array_element(),
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
            Opcode::Unwrap => self.exec_unwrap(),
            Opcode::LoadObjectField => self.exec_load_object_field(),
            Opcode::StoreObjectField => self.exec_store_object_field(),
        }
    }

    fn set_stack_value(&mut self, index: usize, value: Value) {
        if index < self.stack.len() {
            self.stack[index] = value;
        } else if index == self.stack.len() {
            self.stack.push(value);
        } else {
            println!("index={}, value={:?}", index, value);
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

    fn resolve_ref<F, T>(&self, value: &Value, mut f: F) -> T
    where
        F: FnMut(&Value) -> T,
    {
        match value {
            Value::Rc(rc) => {
                let value = &*rc.borrow();
                self.resolve_ref(value, f)
            }
            Value::Ref(obj_id) => {
                let value = self.heap.fetch_obj(*obj_id);
                self.resolve_ref(value, f)
            }
            _ => f(value),
        }
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
        let obj_id = self.heap.alloc_obj(Value::String(str));
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

    fn exec_push_method(&mut self) {
        let obj = self.op_stack.pop().unwrap();
        let class_id = self.read_u32() as usize;
        let method_id = self.read_u32() as usize;
        let func_id = self.classes[class_id].methods[method_id];
        let type_id = self.functions[func_id.0].type_id;
        self.op_stack.push(Value::Method {
            type_id,
            obj: obj.into(),
            func_kind: FnKind::Verse {
                id: func_id,
                upvalues: vec![],
            },
        });
    }

    fn exec_store_local(&mut self) {
        let value = self.op_stack.last().unwrap().copy_value();
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
        let value = self.op_stack.last().unwrap().copy_value();
        let index = self.read_u32() as usize;
        self.set_stack_value(index, value);
    }

    fn exec_load_global(&mut self) {
        let index = self.read_u32() as usize;
        let value = self.stack[index].clone();
        self.op_stack.push(value);
    }

    fn exec_store_upvalue(&mut self) {
        let value = self.op_stack.last().unwrap().copy_value();
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
        let value = Value::Rc(Rc::new(value.into()));
        self.op_stack.push(value);
    }

    fn exec_make_array(&mut self) {
        let type_id = self.read_u32();
        let elem_cnt = self.read_u32();
        let start = self.op_stack.len() - elem_cnt as usize;
        let elements = self.op_stack.split_off(start);
        let obj_id = self.heap.alloc_obj(Value::Array {
            type_id: TypeId(type_id),
            elements,
        });
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_index_tuple(&mut self) {
        let elem_idx = self.read_byte() as usize;
        let value = match self.op_stack.pop().unwrap() {
            Value::Rc(v) => match &*v.borrow() {
                Value::Tuple { elements, .. } => elements[elem_idx].clone(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        self.op_stack.push(value);
    }

    fn exec_set_tuple_element(&mut self) {
        let elem_idx = self.read_byte() as usize;
        let tuple = self.op_stack.pop().unwrap();
        let elem = self.op_stack.last().unwrap().copy_value();
        match tuple {
            Value::Rc(rc) => match &mut *rc.borrow_mut() {
                Value::Tuple { elements, .. } => {
                    elements[elem_idx] = elem;
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
    }

    fn exec_index_array(&mut self) {
        let index = match self.op_stack.pop().unwrap() {
            Value::Integer(v) => {
                if v >= 0 {
                    v as usize
                } else {
                    self.has_pending_failure = true;
                    return;
                }
            }
            _ => unreachable!(),
        };
        let array = self.op_stack.pop().unwrap();
        let element = self.resolve_ref(&array, |arr| match arr {
            Value::Array { elements, .. } => elements.get(index).cloned(),
            _ => unreachable!(),
        });
        if let Some(element) = element {
            self.op_stack.push(element);
        } else {
            self.has_pending_failure = true;
        }
    }

    fn exec_set_array_element(&mut self) {
        let index = match self.op_stack.pop().unwrap() {
            Value::Integer(v) => v,
            _ => unreachable!(),
        };
        let array_ref = self.op_stack.pop().unwrap();
        let value = self.op_stack.last().unwrap().copy_value();
        match array_ref {
            Value::Ref(obj_id) => match self.heap.fetch_obj_mut(obj_id) {
                Value::Array { elements, .. } => {
                    if index >= 0 && (index as usize) < elements.len() {
                        elements[index as usize] = value;
                    } else {
                        self.has_pending_failure = true
                    }
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
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

    fn exec_make_object(&mut self) {
        let type_id = self.read_u32();
        let class_id = self.read_u32();
        let field_count = self.read_u32() as usize;
        let method_count = self.read_u32() as usize;
        let methods = self.op_stack.split_off(self.op_stack.len() - method_count);
        let fields = self.op_stack.split_off(self.op_stack.len() - field_count);
        let obj_id = self.heap.alloc_obj(Value::Object {
            type_id: TypeId(type_id),
            class_id,
            fields,
            methods,
        });
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_load_object_field(&mut self) {
        let obj = self.op_stack.pop().unwrap();
        let field_index = self.read_u32() as usize;
        match obj {
            Value::Ref(obj_id) => match self.heap.fetch_obj(obj_id) {
                Value::Object { fields, .. } => self.op_stack.push(fields[field_index].clone()),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    fn exec_store_object_field(&mut self) {
        let obj = self.op_stack.pop().unwrap();
        let value = self.op_stack.last().unwrap().copy_value();
        let field_index = self.read_u32() as usize;
        match obj {
            Value::Ref(obj_id) => match self.heap.fetch_obj_mut(obj_id) {
                Value::Object { fields, .. } => {
                    fields[field_index] = value;
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    fn exec_call(&mut self) {
        let argc = self.read_u32();
        let args = self.op_stack.split_off(self.op_stack.len() - argc as usize);
        let callee = self.op_stack.pop().unwrap();
        let func = match callee {
            Value::Ref(obj_id) => match self.heap.fetch_obj(obj_id) {
                Value::Function { kind, .. } => kind,
                _ => unreachable!(),
            },
            Value::Method { ref func_kind, .. } => func_kind,
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
            self.resolve_ref(&value, |v| match v {
                Value::String(s) => buf.push_str(s),
                _ => panic!("not string"),
            });
        }
        let obj_id = self.heap.alloc_obj(Value::String(buf));
        self.op_stack.push(Value::Ref(obj_id));
    }

    fn exec_cast(&mut self) {
        let expect = TypeId(self.read_u32());
        let value = self.op_stack.last().unwrap();
        let type_id = self.resolve_ref(value, |v| match v {
            Value::Void => self.predefined_types.t_void,
            Value::Integer(_) => self.predefined_types.t_int,
            Value::Rational(..) => self.predefined_types.t_rational,
            Value::Float(_) => self.predefined_types.t_float,
            Value::Char(_) => self.predefined_types.t_char,
            Value::Char32(_) => self.predefined_types.t_char32,
            Value::String(_) => self.predefined_types.t_string,
            Value::Logic(_) => self.predefined_types.t_logic,
            Value::Option { type_id, .. }
            | Value::Tuple { type_id, .. }
            | Value::Array { type_id, .. }
            | Value::Object { type_id, .. }
            | Value::Function { type_id, .. }
            | Value::Method { type_id, .. }
            | Value::Type(type_id) => *type_id,
            Value::Rc(_) | Value::Ref(_) => unreachable!(),
        });
        self.has_pending_failure = expect != type_id
    }

    fn exec_len(&mut self) {
        let value = self.op_stack.pop().unwrap();
        let len = self.resolve_ref(&value, |v| match v {
            Value::String(s) => s.len(),
            Value::Array { elements, .. } => elements.len(),
            _ => unreachable!(),
        });
        self.op_stack.push(Value::Integer(len as i64));
    }

    fn exec_unwrap(&mut self) {
        let inner = match self.op_stack.pop().unwrap() {
            Value::Option { value, .. } => value,
            _ => panic!("cannot unwrap non option value"),
        };

        if let Some(value) = inner {
            self.op_stack.push(*value);
        } else {
            self.has_pending_failure = true;
        }
    }
}
