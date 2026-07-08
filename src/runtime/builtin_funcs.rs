use std::fmt::Write;

use crate::runtime::{CallContext, Value, heap::Heap};

pub fn write_value(
    w: &mut impl Write,
    heap: &dyn Heap,
    value: &Value,
    quate_string: bool,
) -> Result<(), std::fmt::Error> {
    match value {
        Value::String(str) => {
            if quate_string {
                write!(w, "\"{}\"", str)
            } else {
                write!(w, "{}", str)
            }
        }
        Value::Void => Ok(()),
        Value::Logic(value) => write!(w, "{}", value),
        Value::Integer(value) => write!(w, "{}", value),
        Value::Rational(num, den) => write!(w, "{}/{}", num, den),
        Value::Float(value) => write!(w, "{}", value),
        Value::Char(value) => write!(w, "{}", *value as char),
        Value::Char32(value) => write!(w, "{}", value),
        Value::Tuple { elements, .. } => {
            write!(w, "(")?;
            for (i, v) in elements.iter().enumerate() {
                if i > 0 {
                    write!(w, ", ")?;
                }
                write_value(w, heap, v, true)?;
            }
            write!(w, ")")
        }
        Value::Function { .. } => write!(w, "[Function]"),
        Value::Type { .. } => write!(w, "[Type]"),
        Value::Option(_) => write!(w, "[Option]"),
        Value::Ref(id) => write!(w, "[Ref({})]", id.0),
    }
}

pub fn print(ctx: &mut CallContext) {
    if let Some(arg) = ctx.args.first() {
        let mut buf = String::new();
        write_value(&mut buf, ctx.heap, arg, false).unwrap();
        println!("{}", buf);
    } else {
        println!();
    }
}
