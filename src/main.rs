use std::env;
use std::fs;
use std::io::{self, Read};

use verse::compiler::{Compiler, lexer::tokenize, parser::Parser, semantic::SemanticAnalyzer};
use verse::debug::{print_parser_error, print_semantic_error};
use verse::vm::Vm;
use verse::vm::global_vars;

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
        let mut semantic_ctx = SemanticAnalyzer::new(&mut parser.symbol_reg);

        let entry = semantic_ctx.analyze(&program.expressions);
        for err in &semantic_ctx.errors {
            print_semantic_error(&err, &source, &parser.symbol_reg);
        }
        if semantic_ctx.errors.is_empty() {
            let mut compiler = Compiler::default();
            let funcs = compiler.compile(entry);
            let mut vm = Vm::new(
                parser.const_pool.into_table(),
                semantic_ctx.predefined_types,
            );
            global_vars::install(
                &mut vm,
                semantic_ctx.builtin_symbols,
                semantic_ctx.predefined_types,
                &mut compiler.type_registry,
                |s| semantic_ctx.get_global_symbol_index(s),
            );
            let main = funcs.len() - 1;
            vm.functions = funcs;
            let value = vm.run(main);
            println!("{:?}", value);
        }
    }
}
