use crate::{core::SymbolTable, lexer::LexerError, parser::ParseError, semantic::SemanticError};

pub fn print_semantic_error(err: &SemanticError, src: &str, symbol_tbl: &SymbolTable) {
    let (span, suffix) = match err {
        SemanticError::Reference { span, symbol } => (
            span,
            format!("cannot find `{}`", symbol_tbl.resolve(*symbol)),
        ),
        SemanticError::TypeMismatch {
            span,
            expect,
            found,
        } => (
            span,
            format!("type mismatched, expect: {:?}, found = {:?}", expect, found),
        ),
        SemanticError::TypeNotFound { span, symbol } => (
            span,
            format!("cannot find type `{}`", symbol_tbl.resolve(*symbol)),
        ),
        SemanticError::Mutability { span, symbol } => (
            span,
            format!(
                "cannot reassign immutable variable `{}`",
                symbol_tbl.resolve(*symbol)
            ),
        ),
        SemanticError::ArgsCountMismatch { span } => (span, format!("arguments count mismatch")),
        SemanticError::UnexpectedExpr {
            span,
            expect: expected,
            found,
        } => (span, format!("expect {}, found {}", expected, found)),
        SemanticError::NotCallable { callee } => (&callee.span, format!("is not callable")),
    };

    let start_pos = get_source_position(src, span.start).unwrap();
    let end_pos = get_source_position(src, span.end).unwrap();
    println!(
        "{}:{}-{}:{} {}",
        start_pos.0, start_pos.1, end_pos.0, end_pos.1, suffix
    );
}

pub fn print_parser_error(err: &ParseError, src: &str) {
    match err {
        ParseError::UnexpectedToken { token, span } => {
            let start_pos = get_source_position(src, span.start).unwrap();
            let end_pos = get_source_position(src, span.end).unwrap();
            println!(
                "{}:{}-{}:{} Unexpected token `{:?}`",
                start_pos.0, start_pos.1, end_pos.0, end_pos.1, token
            )
        }
        ParseError::InvalidToken { token, span } => {
            let start_pos = get_source_position(src, span.start).unwrap();
            let end_pos = get_source_position(src, span.end).unwrap();
            println!(
                "{}:{}-{}:{} Invalid token `{:?}`",
                start_pos.0, start_pos.1, end_pos.0, end_pos.1, token
            )
        }
        ParseError::LexerError { inner, .. } => match inner {
            LexerError::InvalidToken(span) => {
                let start_pos = get_source_position(src, span.start).unwrap();
                let end_pos = get_source_position(src, span.end).unwrap();
                println!(
                    "{}:{}-{}:{} Invalid token `{}`",
                    start_pos.0,
                    start_pos.1,
                    end_pos.0,
                    end_pos.1,
                    &src[span.clone()]
                )
            }
            _ => unimplemented!(),
        },
        ParseError::SyntaxError { message, span } => {
            let start_pos = get_source_position(src, span.start).unwrap();
            let end_pos = get_source_position(src, span.end).unwrap();
            println!(
                "{}:{}-{}:{} {}",
                start_pos.0, start_pos.1, end_pos.0, end_pos.1, message
            )
        }
    }
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
