use std::env;
use std::fs;
use std::io::{self, Read};

use verse::compiler::{
    compiler::Compiler, lexer::tokenize, parser::Parser, semantic::SemanticAnalyzer,
};
use verse::diagnostic::{report_parser_error, report_semantic_error};
use verse::vm::Vm;
use verse::vm::global_vars;

fn main() {
    let mut filename = env::args().skip(1).next().expect("no filename provided!");
    let mut source = String::new();

    if filename == "-" {
        io::stdin().read_to_string(&mut source).unwrap();
        filename = "stdin".to_string();
    } else {
        source = fs::read_to_string(&filename).unwrap();
        filename = fs::canonicalize(filename)
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();
    }

    let tokens = tokenize(&source).unwrap();
    let mut parser = Parser::new(&source, &tokens);

    let program = parser.parse();
    for err in &parser.errors {
        report_parser_error(err, &source, &filename);
    }
    if parser.errors.is_empty() {
        let mut analyzer = SemanticAnalyzer::new(&mut parser.symbol_table);

        let root_irs = analyzer.analyze(&program.expressions);
        for err in &analyzer.errors {
            report_semantic_error(&err, &source, &filename);
        }
        if analyzer.errors.is_empty() {
            let mut compiler = Compiler::new();
            let funcs = compiler.compile(root_irs);
            let mut vm = Vm::new(parser.const_pool.into_vec(), compiler.predefined_types);
            global_vars::install(
                &mut vm,
                analyzer.builtin_symbols,
                compiler.predefined_types,
                &mut compiler.type_registry,
                |s| analyzer.get_global_symbol_index(s),
            );
            let entry_fn_id = funcs.len() - 1;
            vm.functions = funcs;
            vm.run(entry_fn_id);
        }
    }
}
