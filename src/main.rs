use std::env;
use std::fs;
use std::io::{self, Read};

use verse::compiler::compile;
use verse::diagnostic::report_compile_error;
use verse::vm::{Vm, global_vars};

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

    match compile(&source) {
        Ok(mut outcome) => {
            let mut vm = Vm::new(outcome.const_table, outcome.predefined_types);
            global_vars::install(
                &mut vm,
                &mut outcome.symbol_table,
                &mut outcome.type_registry,
                outcome.global_symbol_slots,
            );
            vm.functions = outcome.functions;
            vm.run(outcome.entry);
        }
        Err(errors) => {
            for err in errors {
                report_compile_error(&err, &source, &filename);
            }
        }
    }
}
