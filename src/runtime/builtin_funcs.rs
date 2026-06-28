use crate::runtime::CallContext;

pub fn print(ctx: &mut CallContext) {
    if let Some(arg) = ctx.args.first() {
        println!("{}", arg);
    } else {
        println!();
    }
}
