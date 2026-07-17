use crate::compiler::{
    ast::{BinaryOp, UnaryOp},
    lexer::LexerError,
    parser::ParseError,
    semantic::SemanticError,
};

pub fn report_semantic_error(err: &SemanticError, src: &str) {
    let (span, message) = match err {
        SemanticError::UndefinedName { span } => (
            span,
            format!("cannot find `{}` in this scope", &src[span.clone()]),
        ),
        SemanticError::TypeMismatch {
            span,
            expected,
            found,
        } => (
            span,
            format!("type mismatch: expected `{expected}`, found `{found}`"),
        ),
        SemanticError::InvalidUnaryOp {
            span,
            op,
            operand,
            expected,
        } => {
            let op_str = fmt_unary_op(op);
            let expected: Vec<String> = expected.iter().map(|t| format!("`{t}`")).collect();
            let expected_str = match expected.len() {
                0 => String::new(),
                1 => format!(" (expected {})", expected[0]),
                _ => format!(" (expected one of: {})", expected.join(", ")),
            };
            (
                span,
                format!("cannot apply unary `{op_str}` to type `{operand}`{expected_str}"),
            )
        }
        SemanticError::InvalidBinaryOp { span, op, lhs, rhs } => (
            span,
            format!(
                "cannot apply `{}` to types `{lhs}` and `{rhs}`",
                fmt_binary_op(op)
            ),
        ),
        SemanticError::ImmutableAssignment { span } => (
            span,
            format!(
                "cannot assign to immutable variable `{}`",
                &src[span.clone()]
            ),
        ),
        SemanticError::InvalidAssignmentTarget { span } => {
            (span, "invalid assignment target".into())
        }
        SemanticError::NotCallable { span, ty } => {
            (span, format!("value of type `{ty}` is not callable"))
        }
        SemanticError::ArgCountMismatch {
            span,
            expected,
            found,
        } => (
            span,
            format!("expected {expected} argument(s), found {found}"),
        ),
        SemanticError::InvalidTupleIndex { span } => (
            span,
            "tuple index must be a non-negative integer literal".into(),
        ),
        SemanticError::TupleIndexOutOfBounds {
            span,
            index,
            length,
        } => (
            span,
            format!("tuple index {index} out of bounds for tuple of length {length}"),
        ),
        SemanticError::ExpectedTypeGotValue { span } => {
            (span, "expected a type, found a value".into())
        }
        SemanticError::BreakOutsideLoop { span } => {
            (span, "`break` is not allowed outside of a loop".into())
        }
        SemanticError::InvalidEffect { span } => (span, "invalid function effect".into()),
        SemanticError::UnexpectedFallibleExpr { span } => (
            span,
            "fallible expression is not allowed in this context".into(),
        ),
        SemanticError::ExpectedFallibleExpr { span } => {
            (span, "expected a fallible expression".into())
        }
    };

    print_diagnostic(src, span, &message);
}

pub fn report_parser_error(err: &ParseError, src: &str) {
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

    print_diagnostic(src, span, &message);
}

fn print_diagnostic(src: &str, span: &std::ops::Range<usize>, message: &str) {
    let (line_no, col) = offset_to_line_col(src, span.start).unwrap_or((1, 1));
    let (line_str, line_start_offset) = get_source_line(src, span.start);

    let gutter_width = format!("{}", line_no).len();

    eprintln!("error: {message}");
    eprintln!("{:>gutter_width$} --> {}:{}", "", line_no, col);
    eprintln!("{:>gutter_width$} |", "");

    let display_line = line_str.trim_end_matches('\n').trim_end_matches('\r');
    eprintln!("{line_no:>gutter_width$} | {display_line}");

    let start_in_line = span.start.saturating_sub(line_start_offset);
    let end_in_line = span
        .end
        .saturating_sub(line_start_offset)
        .max(start_in_line + 1);
    let underline_len = (end_in_line - start_in_line)
        .min(display_line.len().saturating_sub(start_in_line))
        .max(1);
    let padding = " ".repeat(start_in_line);
    eprintln!(
        "{:>gutter_width$} | {padding}{}",
        "",
        "^".repeat(underline_len)
    );
}

fn fmt_unary_op(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Plus => "+",
        UnaryOp::Minus => "-",
        UnaryOp::Not => "not",
    }
}

fn fmt_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
    }
}

fn offset_to_line_col(src: &str, offset: usize) -> Option<(usize, usize)> {
    let mut line = 1;
    let mut col = 1;
    let mut current = 0;

    if offset == 0 {
        return Some((line, col));
    }

    for ch in src.chars() {
        let char_len = ch.len_utf8();
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        current += char_len;
        if current == offset {
            return Some((line, col));
        }
    }

    None
}

fn get_source_line(src: &str, offset: usize) -> (&str, usize) {
    let line_start = src[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line_end = src[offset..].find('\n').map_or(src.len(), |p| offset + p);
    (&src[line_start..line_end], line_start)
}
