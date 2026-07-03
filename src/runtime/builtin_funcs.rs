use crate::runtime::{CallContext, Value};

pub fn print(ctx: &mut CallContext) {
    if let Some(arg) = ctx.args.first() {
        match arg {
            Value::String(s) => println!("{}", s),
            _ => println!("{}", arg),
        }
    } else {
        println!();
    }
}
