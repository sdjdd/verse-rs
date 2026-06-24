use logos::Logos;
use verse::eval::{EvalContext, Value, eval};
use verse::{lexer::Token, parser::Parser};

fn main() {
    let source = r#"
        "\<"
    "#;
    let lexer = Token::lexer(source);
    let mut parser = Parser::new(lexer);
    let mut ctx = EvalContext::new();
    let mut value = Value::None;
    parser.parse().unwrap().expressions.iter().for_each(|expr| {
        // println!("{:#?}", expr);
        value = eval(expr, &mut ctx).unwrap();
    });
    println!("{}", value);
}
