use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::{
    self,
    termcolor::{ColorChoice, StandardStream},
};

use crate::compiler::{lexer::LexerError, parser::ParseError, semantic::SemanticError};

pub fn report_semantic_error(err: &SemanticError, src: &str, filename: &str) {
    print_diagnostic(filename, src, &err.span, &err.kind.to_string());
}

pub fn report_parser_error(err: &ParseError, src: &str, filename: &str) {
    let (span, message) = match err {
        ParseError::UnexpectedToken { token, span } => {
            (span, format!("unexpected token `{token:?}`"))
        }
        ParseError::InvalidToken { token, span } => (span, format!("invalid token `{token:?}`")),
        ParseError::LexerError { inner, .. } => match inner {
            LexerError::InvalidToken(span) => {
                (span, format!("invalid token `{}`", &src[span.clone()]))
            }
            _ => unimplemented!(),
        },
        ParseError::SyntaxError { message, span } => (span, message.clone()),
    };

    print_diagnostic(filename, src, span, &message);
}

fn print_diagnostic(filename: &str, src: &str, span: &std::ops::Range<usize>, message: &str) {
    let file = SimpleFile::new(filename, src);

    let diagnostic = Diagnostic::error()
        .with_message(message)
        .with_labels(vec![Label::primary((), span.clone())]);

    let writer = StandardStream::stderr(ColorChoice::Always);
    let config = term::Config {
        chars: term::Chars::ascii(),
        ..Default::default()
    };
    term::emit_to_io_write(&mut writer.lock(), &config, &file, &diagnostic).unwrap();
}
