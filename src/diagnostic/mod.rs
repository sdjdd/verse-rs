use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::{
    self,
    termcolor::{ColorChoice, StandardStream},
};

use crate::compiler::parser::ParseError;
use crate::compiler::{CompileError, CompileErrorKind};

pub fn report_parser_error(err: &ParseError, src: &str, filename: &str) {
    let (span, message) = match err {
        ParseError::UnexpectedToken { token, span } => {
            (span, format!("unexpected token `{token:?}`"))
        }
        ParseError::InvalidExpression { span } => (span, err.to_string()),
    };

    print_diagnostic(filename, src, span, &message);
}

pub fn report_compile_error(err: &CompileError, src: &str, filename: &str) {
    match &err.kind {
        CompileErrorKind::Lexing(e) => print_diagnostic(filename, src, &err.span, &e.to_string()),
        CompileErrorKind::Parsing(e) => report_parser_error(e, src, filename),
        CompileErrorKind::Semantic(e) => print_diagnostic(filename, src, &err.span, &e.to_string()),
    }
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
