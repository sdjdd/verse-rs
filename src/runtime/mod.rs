#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Integer(i64),
    Float(f64),
    Char(u8),
    Char32(char),
    String(String),
    Logic(bool),
    Tuple(Vec<Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Void => Ok(()),
            Value::Logic(value) => write!(f, "{}", value),
            Value::Integer(value) => write!(f, "{}", value),
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
            _ => unimplemented!(),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            _ => unimplemented!(),
        }
    }
}
