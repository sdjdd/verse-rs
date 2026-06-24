use logos::Logos;
use verse::eval::{EvalContext, Value, eval};
use verse::{lexer::Token, parser::Parser};

fn main() {
    let source = r#"
        1 = 1
    "#;
    let lexer = Token::lexer(source);
    let mut parser = Parser::new(lexer);
    let mut ctx = EvalContext::new();
    let mut value = Ok(Value::None);
    parser.parse().unwrap().expressions.iter().for_each(|expr| {
        // println!("{:#?}", expr);
        value = eval(expr, &mut ctx).unwrap();
    });
    println!("{:?}", value);
}
