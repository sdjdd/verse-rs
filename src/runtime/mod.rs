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

#[derive(Debug, Clone, Copy)]
pub enum FunctionKind {
    Native(NativeFunction),
    Verse(FunctionId),
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct TypeId(pub usize);

#[derive(Debug, Clone, Copy)]
pub enum Value {
    Void,
    Integer(i64),
    Rational(i64, i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(ObjectId),
    Logic(bool),

    Tuple { ty: TypeId, oid: ObjectId },
    Function { kind: FunctionKind },
    Type(TypeId),
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

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}
