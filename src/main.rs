use std::env;
use std::fs;
use std::io::{self, Read};

use verse::eval::{EvalContext, eval};
use verse::lexer::IndentAwareLexer;
use verse::parser::{ParseError, Parser};
use verse::runtime::Value;

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
    let mut ctx = EvalContext::new();
    let program = parser
        .parse()
        .map_err(|err| {
            match &err {
                ParseError::UnexpectedToken { span, .. } => {
                    println!(
                        "line,column 1 = {:?}",
                        get_source_position(&source, span.start)
                    );
                    println!(
                        "line,column 2 = {:?}",
                        get_source_position(&source, span.end)
                    );
                }
                _ => {}
            };
            err
        })
        .unwrap();
    let mut value = Ok(Value::Void);
    program.expressions.iter().for_each(|expr| {
        value = eval(expr, &mut ctx).unwrap();
    });
    println!("{:?}", value);
}

fn get_source_position(src: &str, offset: usize) -> Option<(usize, usize)> {
    let mut line: usize = 1;
    let mut col: usize = 1;
    let mut ofst: usize = 0;

    if offset == ofst {
        return Some((line, col));
    }

    for ch in src.chars() {
        ofst += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        if offset == ofst {
            return Some((line, col));
        }
    }

    None
}
