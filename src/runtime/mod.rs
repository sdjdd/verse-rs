use derive_more::From;

use crate::runtime::heap::{Heap, ObjectId};

pub mod builtin_funcs;
pub mod heap;

#[derive(Debug)]
pub struct Failure();

pub struct CallContext<'a> {
    pub heap: &'a dyn Heap,
    pub args: &'a [Value],
    pub ret_val: Option<Result<Value, Failure>>,
}

pub type NativeFunction = fn(ctx: &mut CallContext);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct FunctionId(pub usize);

#[derive(Debug, Clone)]
pub enum FnKind {
    Native(NativeFunction),
    Verse {
        id: FunctionId,
        upvalues: Vec<ObjectId>,
    },
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, From)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Default)]
pub enum Value {
    #[default]
    Void,
    Integer(i64),
    Rational(i64, i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    False,
    Logic(bool),
    Option {
        type_id: TypeId,
        value: Option<Box<Value>>,
    },
    Tuple {
        type_id: TypeId,
        elements: Vec<Value>,
    },
    Array {
        type_id: TypeId,
        elements: Vec<Value>,
    },
    Function {
        type_id: TypeId,
        kind: FnKind,
    },
    Type(TypeId),
    Ref(ObjectId),
}

impl Value {
    pub fn rational(num: i64, den: i64) -> Self {
        assert!(den != 0);
        let g = gcd(num.unsigned_abs(), den.unsigned_abs()) as i64;
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        if d == 1 {
            Value::Integer(n)
        } else {
            Value::Rational(n, d)
        }
    }

    pub fn to_rational(&self) -> Option<(i64, i64)> {
        match self {
            Value::Integer(n) => Some((*n, 1)),
            Value::Rational(n, d) => Some((*n, *d)),
            _ => None,
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Value::Integer(v) => *v == 0,
            Value::Float(v) => *v == 0.0,
            Value::Rational(v, ..) => *v == 0,
            _ => unimplemented!(),
        }
    }

    pub fn to_string(self: Value) -> String {
        match self {
            Value::Integer(v) => format!("{}", v),
            Value::Float(v) => format!("{}", v),
            Value::Char(c) => format!("{}", c as char),
            Value::Char32(c) => format!("{}", c),
            Value::String(s) => s,
            _ => unimplemented!(),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.eq(b),
            (Value::Integer(n), Value::Rational(num, den))
            | (Value::Rational(num, den), Value::Integer(n)) => n * den == *num,
            (Value::Rational(a_n, a_d), Value::Rational(b_n, b_d)) => a_n * b_d == *b_n * a_d,
            _ => unimplemented!(),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Integer(n), Value::Rational(num, den))
            | (Value::Rational(num, den), Value::Integer(n)) => (n * den).partial_cmp(num),
            (Value::Rational(a_n, a_d), Value::Rational(b_n, b_d)) => {
                (a_n * b_d).partial_cmp(&(b_n * a_d))
            }
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(a + b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
            (Value::Rational(a1, a2), Value::Rational(b1, b2)) => {
                Value::Rational(a1 * b2 + b1 * a2, a2 * b2)
            }
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(a - b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(a * b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Div for Value {
    type Output = Value;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(a / b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Neg for Value {
    type Output = Value;

    fn neg(self) -> Self::Output {
        match self {
            Value::Integer(v) => Value::Integer(-v),
            Value::Float(v) => Value::Float(-v),
            Value::Rational(n, d) => Value::Rational(-n, d),
            _ => panic!("invalid Neg operand"),
        }
    }
}

impl std::ops::Not for Value {
    type Output = Value;

    fn not(self) -> Self::Output {
        match self {
            Value::Logic(v) => Value::Logic(!v),
            _ => panic!("invalid Not operand"),
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}
