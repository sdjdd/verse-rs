use logos::{Logos, Skip};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenInfo {
    pub indent: usize,
}

#[derive(Clone, Copy, Default)]
pub struct LexerState {
    indent: usize,
}

#[derive(Default, Debug, PartialEq, Clone)]
pub enum LexerError {
    #[default]
    Unknown,
    InvalidToken(String),
}

impl LexerError {
    fn from_lexer(lex: &mut Lexer) -> Self {
        Self::InvalidToken(lex.slice().to_string())
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
// #[logos(skip r"[ \t\n\f]")]
#[logos(extras = LexerState)]
#[logos(error(LexerError, LexerError::from_lexer))]
pub enum Token {
    #[regex(r"[ \t]+", whitespace_callback)]
    Whitespace,

    #[regex(r"\r?\n", newline_callback)]
    Newline,

    #[token(":=")]
    ColonEq,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token(",")]
    Comma,

    #[token("+")]
    Plus,

    #[token("-")]
    Minus,

    #[token("*")]
    Star,

    #[token("/")]
    Slash,

    #[regex("[0-9]+", integer_callback)]
    #[regex("0x[0-9A-Fa-f]+", integer_callback_hex)]
    IntegerLiteral(i64),

    #[regex(r"[0-9]+\.[0-9]+(e[+-]?[0-9]+)?(f64)?")]
    FloatLiteral,

    #[regex(r"'[\x{00}-\x{FF}]'", |lex| char_callback_basic(lex) as u8)]
    #[regex(r#"'\\[tnr"'\\{}<>&#~]'"#, |lex| char_callback_escaped(lex) as u8)]
    #[regex(r"0o[0-7]{2}", |lex| char_callback_hex(lex).map(|v| v as u8))]
    CharLiteral(u8),

    #[regex(r"'[\x{100}-\x{10FFFF}]'", char_callback_basic)]
    #[regex(r"0u[0-9A-Fa-f]{5}", char_callback_hex)]
    Char32Literal(char),

    #[token("true")]
    True,

    #[token("false")]
    False,

    #[token("if")]
    If,

    #[token("then")]
    Then,

    #[token("else")]
    Else,

    #[regex("[a-zA-Z_]+[a-zA-Z0-9_]*")]
    Ident,

    EOF,
}

pub type Lexer<'src> = logos::Lexer<'src, Token>;

fn whitespace_callback(lex: &mut Lexer) -> Skip {
    lex.extras.indent += lex.span().count();
    Skip
}

fn newline_callback(lex: &mut Lexer) -> Skip {
    lex.extras.indent = 0;
    Skip
}

fn map_parse_int_err(lex: &Lexer, err: std::num::ParseIntError) -> LexerError {
    use std::num::IntErrorKind;
    match err.kind() {
        IntErrorKind::PosOverflow => LexerError::InvalidToken("Number is too large".to_string()),
        IntErrorKind::NegOverflow => LexerError::InvalidToken("Number is too small".to_string()),
        _ => LexerError::InvalidToken(lex.slice().to_string()),
    }
}

fn integer_callback(lex: &mut Lexer) -> Result<i64, LexerError> {
    lex.slice()
        .parse()
        .map_err(|err| map_parse_int_err(&lex, err))
}

fn integer_callback_hex(lex: &mut Lexer) -> Result<i64, LexerError> {
    i64::from_str_radix(&lex.slice()[2..], 16).map_err(|err| map_parse_int_err(&lex, err))
}

fn char_callback_basic(lex: &mut Lexer) -> char {
    lex.slice().chars().nth(1).unwrap()
}

fn char_callback_escaped(lex: &mut Lexer) -> char {
    match lex.slice().chars().nth(2).unwrap() {
        't' => '\u{0009}',
        'n' => '\u{000A}',
        'r' => '\u{000D}',
        '"' => '\u{0022}',
        '\'' => '\u{0027}',
        '\\' => '\u{005C}',
        '{' => '\u{007B}',
        '}' => '\u{007D}',
        '<' => '\u{003C}',
        '>' => '\u{003E}',
        '&' => '\u{0026}',
        '#' => '\u{0023}',
        '~' => '\u{007E}',
        _ => unreachable!(),
    }
}

fn char_callback_hex(lex: &mut Lexer) -> Result<char, LexerError> {
    let value = u32::from_str_radix(&lex.slice()[2..], 16).unwrap();
    std::char::from_u32(value).ok_or_else(|| LexerError::InvalidToken(lex.slice().to_string()))
}
