use logos::Logos;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenInfo {
    pub indent: usize,
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
#[logos(error(LexerError, LexerError::from_lexer))]
#[logos(subpattern escape = r#"[tnr"'\\{}<>&#~]"#)]
#[logos(subpattern string = r#"([^(?&escape)]|\\.)*"#)]
pub enum Token {
    #[regex(r"[ ]+")]
    Whitespace,

    #[regex(r"[\t]+")]
    Tabs,

    #[regex(r"\r?\n")]
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

    #[regex("[0-9]+")]
    #[regex("0x[0-9A-Fa-f]+")]
    IntegerLiteral,

    #[regex(r"[0-9]+\.[0-9]+(e[+-]?[0-9]+)?(f64)?")]
    FloatLiteral,

    #[regex(r"'[\x{00}-\x{7F}]'")]
    #[regex(r#"'\\(?&escape)'"#)]
    #[regex(r"0o[0-9A-Fa-f]{2}")]
    CharLiteral,

    #[regex(r"'[\x{80}-\x{10FFFF}]'")]
    #[regex(r"0u[0-9A-Fa-f]{5}")]
    Char32Literal,

    #[regex(r#""(?&string)""#)]
    StringLiteral,

    #[regex(r#""(?&string)\{"#)]
    TemplateHead,

    #[regex(r#"\}(?&string)\{"#)]
    TemplateMiddle,

    #[regex(r#"\}(?&string)""#)]
    TemplateTail,

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
