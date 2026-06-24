use logos::Logos;
use verse::eval::{EvalContext, eval};
use verse::lexer::Token;
use verse::parser::Parser;
use verse::runtime::Value;

fn main() {
    let source = r#"
        X := 10
        5 <= X <= 100 <= 1000 = 1001 - 1 > 1
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
