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

    let program = parser.parse();
    for err in &parser.errors {
        print_parser_error(err, &source);
    }
    if parser.errors.is_empty() {
        let mut semantic_ctx = SemanticAnalyzer::new(&mut parser.symbol_table);

        let entry = semantic_ctx.analyze(&program.expressions);
        for err in &semantic_ctx.errors {
            print_semantic_error(&err, &source, &parser.symbol_table, &semantic_ctx.types);
        }
        if semantic_ctx.errors.is_empty() {
            let mut ctx = Evaluator::new(
                semantic_ctx.builtin_symbols,
                semantic_ctx.predefined_types,
                parser.const_pool.into_table(),
                &semantic_ctx.scopes[0],
            );
            let mut value = Ok(Value::Void);
            entry.into_iter().for_each(|ir| value = ctx.eval(&ir));
            println!("{:?}", value);
        }
    }
}
