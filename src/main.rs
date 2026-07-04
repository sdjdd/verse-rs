use std::env;
use std::fs;
use std::io::{self, Read};

use verse::debug::{print_parser_error, print_semantic_error};
use verse::eval::Evaluator;
use verse::lexer::tokenize;
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

    let tokens = tokenize(&source).unwrap();
    let mut parser = Parser::new(&source, &tokens);

    let program = parser
        .parse()
        .map_err(|err| print_parser_error(&err, &source))
        .unwrap();

    // println!("{:#?}", program.expressions);

    let mut semantic_ctx = SemanticAnalyzer::new(parser.get_symbol_table_mut());

    let entry = semantic_ctx.analyze(&program.expressions);
    for err in &semantic_ctx.errors {
        print_semantic_error(&err, &source, parser.get_symbol_table());
    }
    if semantic_ctx.errors.is_empty() {
        let mut ctx = Evaluator::new(
            semantic_ctx.builtin_symbols,
            semantic_ctx.builtin_types,
            parser.const_pool.into_table(),
            semantic_ctx.irs.clone(),
        );
        let mut value = Ok(Value::Void);
        entry.into_iter().for_each(|ir| value = ctx.eval(ir));
        println!("{:?}", value);
    }
}
