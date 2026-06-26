use std::{collections::VecDeque, ops::Range};

use logos::{Logos, SpannedIter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenInfo {
    pub indent: usize,
}

#[derive(Default, Debug, PartialEq, Clone)]
pub enum LexerError {
    #[default]
    Unknown,
    InvalidToken(String),
    InvalidIndentSize,
    InconsistentIndent,
}

impl LexerError {
    fn from_lexer(lex: &mut Lexer) -> Self {
        Self::InvalidToken(lex.slice().to_string())
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(error(LexerError, LexerError::from_lexer))]
#[logos(subpattern escape = r#"[tnr"'\\{}<>&#~]"#)]
#[logos(subpattern string = r#"([^"{}]|\\.)*"#)]
pub enum Token {
    #[regex(r"[ ]+")]
    Whitespaces,

    #[regex(r"[\t]+")]
    Tabs,

    #[regex(r"\r?\n")]
    Newline,

    #[token(":=")]
    ColonEq,

    #[token(":")]
    Colon,

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

    #[token("<>")]
    NotEq,

    #[token("<=")]
    LessEq,

    #[token(">=")]
    GreaterEq,

    #[token("<")]
    Less,

    #[token(">")]
    Greater,

    #[token("=")]
    Eq,

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

    #[regex("[A-Za-z][A-Za-z0-9_]*")]
    #[regex("_[A-Za-z0-9_]+")]
    Id,

    // Synthetic tokens - injected by IndentAwareLexer, never produced by logos
    Indent,
    Dedent,

    EOF,
}

pub type Lexer<'src> = logos::Lexer<'src, Token>;

pub type Span = Range<usize>;

#[derive(Clone, Copy)]
enum IndentType {
    Space,
    Tab,
}

#[derive(Clone)]
struct IndentInfo {
    typ: IndentType,
    span: Span,
}

#[derive(Clone)]
pub struct IndentAwareLexer<'src> {
    lexer: SpannedIter<'src, Token>,
    pending: VecDeque<(Result<Token, LexerError>, logos::Span)>,
    indent_stack: Vec<IndentInfo>,
    current_indent: Option<IndentInfo>,
    at_line_start: bool,
    has_error: bool,
}

impl<'src> IndentAwareLexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            lexer: Token::lexer(source).spanned(),
            pending: VecDeque::new(),
            indent_stack: vec![],
            current_indent: None,
            at_line_start: true,
            has_error: false,
        }
    }
}

impl<'src> Iterator for IndentAwareLexer<'src> {
    type Item = (Result<Token, LexerError>, logos::Span);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.pending.is_empty() {
            return self.pending.pop_front();
        }

        loop {
            if self.has_error {
                break;
            }

            let (token, span) = match self.lexer.next() {
                Some((Ok(token), span)) => (token, span),
                Some(t @ (Err(_), _)) => {
                    self.pending.push_back(t);
                    self.has_error = true;
                    break;
                }
                None => break,
            };

            match token {
                Token::Newline => {
                    if self.at_line_start {
                        // strip empty line
                        continue;
                    } else {
                        self.current_indent = None;
                        self.at_line_start = true;
                    }
                }
                Token::Whitespaces | Token::Tabs => {
                    if self.at_line_start {
                        self.current_indent = Some(IndentInfo {
                            typ: match token {
                                Token::Whitespaces => IndentType::Space,
                                Token::Tabs => IndentType::Tab,
                                _ => unreachable!(),
                            },
                            span: span.clone(),
                        })
                    } else if let Some(indent) = self.current_indent.as_mut() {
                        indent.span.end = span.end;
                        match (indent.typ, token) {
                            (IndentType::Space, Token::Tabs)
                            | (IndentType::Tab, Token::Whitespaces) => {
                                self.has_error = true;
                                return Some((Err(LexerError::InconsistentIndent), span));
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
                _ => {
                    if self.at_line_start {
                        if let Some(current_indent) = self.current_indent.take() {
                            let last_size = self
                                .indent_stack
                                .last()
                                .cloned()
                                .map(|v| v.span.count())
                                .unwrap_or(0);

                            let current_size = current_indent.span.clone().count();

                            if current_size > last_size {
                                self.indent_stack.push(IndentInfo {
                                    typ: current_indent.typ,
                                    span: current_indent.span.clone(),
                                });
                                self.pending
                                    .push_back((Ok(Token::Indent), current_indent.span.clone()));
                            } else if current_size < last_size {
                                if let Some(pos) = self
                                    .indent_stack
                                    .iter()
                                    .rposition(|v| v.span.clone().count() == current_size)
                                {
                                    for _ in pos + 1..self.indent_stack.len() {
                                        self.pending.push_back((
                                            Ok(Token::Dedent),
                                            current_indent.span.clone(),
                                        ));
                                    }
                                    self.indent_stack.truncate(pos + 1);
                                } else {
                                    self.has_error = true;
                                    self.pending.clear();
                                    return Some((
                                        Err(LexerError::InvalidIndentSize),
                                        current_indent.span.clone(),
                                    ));
                                }
                            }
                        } else if !self.indent_stack.is_empty() {
                            for _ in &self.indent_stack {
                                self.pending
                                    .push_back((Ok(Token::Dedent), span.start..span.start)); // zero size dedent
                            }
                            self.indent_stack.clear();
                        }
                    }

                    self.at_line_start = false;
                }
            }

            self.pending.push_back((Ok(token), span));
            break;
        }

        self.pending.pop_front()
    }
}
