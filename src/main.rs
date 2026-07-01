use std::env;
use std::fs;
use std::io::{self, Read};

use verse::debug::{print_parser_error, print_semantic_error};
use verse::eval::Evaluator;
use verse::lexer::IndentAwareLexer;
use verse::parser::Parser;
use verse::runtime::Value;
use verse::semantic::SemanticAnalyzer;

fn main() {
    let args: Vec<String> = env::args().collect();
    let source = if args.len() > 1 && &args[1] != "-" {
        fs::read_to_string(&args[1]).unwrap()
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).unwrap();
        buffer
    };
    let lexer = IndentAwareLexer::new(&source);
    // for token in lexer.clone().into_iter() {
    //     println!("{:?}", token)
    // }
    let mut parser = Parser::new(&source, lexer);

    let program = parser
        .parse()
        .map_err(|err| print_parser_error(&err, &source))
        .unwrap();

    // println!("{:#?}", program.expressions);

    let mut semantic_ctx = SemanticAnalyzer::new(parser.get_symbol_table_mut());

    for expr in &program.expressions {
        semantic_ctx.handle_expr(expr)
    }
    for err in &semantic_ctx.errors {
        print_semantic_error(&err, &source, parser.get_symbol_table().clone());
    }
    if semantic_ctx.errors.is_empty() {
        let mut ctx = Evaluator::new(
            parser.get_symbol_table().clone(),
            semantic_ctx.get_void_functions(),
        );
        let mut value = Ok(Value::Void);
        program
            .expressions
            .iter()
            .for_each(|expr| value = ctx.eval(expr));
        println!("{:?}", value);
    }
}
