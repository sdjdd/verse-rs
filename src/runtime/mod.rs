#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Integer(i64),
    Rational(i64, i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    Logic(bool),
    Tuple(Vec<Value>),
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
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Void => Ok(()),
            Value::Logic(value) => write!(f, "{}", value),
            Value::Integer(value) => write!(f, "{}", value),
            Value::Rational(num, den) => write!(f, "{}/{}", num, den),
            Value::Float(value) => write!(f, "{}", value),
            Value::Char(value) => write!(f, "{}", *value as char),
            Value::Char32(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "{}", value),
            Value::Tuple(value) => {
                write!(f, "(")?;
                for (i, v) in value.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }
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
            | (Value::Rational(num, den), Value::Integer(n)) => {
                (n * den).partial_cmp(num)
            }
            (Value::Rational(a_n, a_d), Value::Rational(b_n, b_d)) => {
                (a_n * b_d).partial_cmp(&(b_n * a_d))
            }
            _ => unimplemented!(),
        }
    }
}
