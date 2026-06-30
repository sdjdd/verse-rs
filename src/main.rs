use std::env;
use std::fs;
use std::io::{self, Read};

use verse::debug::{print_parser_error, print_semantic_error};
use verse::eval::{EvalContext, eval};
use verse::lexer::IndentAwareLexer;
use verse::parser::Parser;
use verse::runtime::Value;
use verse::semantic::{SemanticContext, resolve_expr_type};

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

    let mut semantic_ctx = SemanticContext::new(parser.get_symbol_table_mut());

    for expr in &program.expressions {
        resolve_expr_type(expr, &mut semantic_ctx)
            .map_err(|e| {
                print_semantic_error(&e, &source, parser.get_symbol_table().clone());
            })
            .unwrap();
    }

    let mut ctx = EvalContext::new(parser.get_symbol_table().clone());
    let mut value = Ok(Value::Void);
    program.expressions.iter().for_each(|expr| {
        value = eval(expr, &mut ctx).unwrap();
    });
    println!("{:?}", value);
}
