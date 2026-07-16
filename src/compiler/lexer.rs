use std::{collections::VecDeque, ops::Range};

use logos::{Lexer, Logos, SpannedIter};

pub type Span = Range<usize>;

#[derive(Default, Debug, PartialEq, Clone)]
pub enum LexerError {
    InvalidToken(Span),
    InvalidIndentSize(Span),
    InconsistentIndent(Span),
    #[default]
    Other,
}

impl LexerError {
    fn from_lexer(lex: &mut Lexer<Token>) -> Self {
        Self::InvalidToken(lex.span().clone())
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(error(LexerError, LexerError::from_lexer))]
#[logos(subpattern escape = r#"[tnr"'\\{}<>&#~]"#)]
#[logos(subpattern string = r#"([^"{}\\\r\n]|\\.)*"#)]
pub enum Token {
    #[regex(r"[ ]+")]
    Whitespaces,

    #[regex(r"[\t]+")]
    Tabs,

    #[regex(r"\r?\n")]
    Newline,

    #[token("_")]
    Underscore,

    #[token(".")]
    Dot,

    #[token("?")]
    Question,

    #[token(":")]
    Colon,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

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

    #[token("not")]
    Not,

    #[token("if")]
    If,

    #[token("then")]
    Then,

    #[token("else")]
    Else,

    #[token("loop")]
    Loop,

    #[token("break")]
    Break,

    #[token("set")]
    Set,

    #[token("var")]
    Var,

    #[token("type")]
    Type,

    #[token("tuple")]
    Tuple,

    #[regex("[A-Za-z][A-Za-z0-9_]*")]
    #[regex("_[A-Za-z0-9_]+")]
    Id,

    // Synthetic tokens - injected by IndentAwareLexer, never produced by logos
    Indent,
    Dedent,

    EOF,
}

#[derive(Clone, Copy, PartialEq)]
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
    pending: VecDeque<Result<(Token, Span), LexerError>>,
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
    type Item = Result<(Token, Span), LexerError>;

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
                Some((Err(err), _)) => {
                    self.pending.push_back(Err(err));
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
                        let new_typ = match token {
                            Token::Whitespaces => IndentType::Space,
                            Token::Tabs => IndentType::Tab,
                            _ => unreachable!(),
                        };

                        if let Some(indent) = &self.current_indent {
                            // Check for inconsistency with existing indent
                            if indent.typ != new_typ {
                                self.has_error = true;
                                return Some(Err(LexerError::InconsistentIndent(span)));
                            }
                            // Extend the span
                            self.current_indent.as_mut().unwrap().span.end = span.end;
                        } else {
                            // First whitespace on this line
                            self.current_indent = Some(IndentInfo {
                                typ: new_typ,
                                span: span.clone(),
                            });
                        }
                    } else if let Some(indent) = self.current_indent.as_mut() {
                        indent.span.end = span.end;
                        match (indent.typ, token) {
                            (IndentType::Space, Token::Tabs)
                            | (IndentType::Tab, Token::Whitespaces) => {
                                self.has_error = true;
                                return Some(Err(LexerError::InconsistentIndent(span)));
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
                                    .push_back(Ok((Token::Indent, current_indent.span.clone())));
                            } else if current_size < last_size {
                                if let Some(pos) = self
                                    .indent_stack
                                    .iter()
                                    .rposition(|v| v.span.clone().count() == current_size)
                                {
                                    for _ in pos + 1..self.indent_stack.len() {
                                        self.pending.push_back(Ok((
                                            Token::Dedent,
                                            current_indent.span.clone(),
                                        )));
                                    }
                                    self.indent_stack.truncate(pos + 1);
                                } else {
                                    self.has_error = true;
                                    self.pending.clear();
                                    return Some(Err(LexerError::InvalidIndentSize(
                                        current_indent.span.clone(),
                                    )));
                                }
                            }
                        } else if !self.indent_stack.is_empty() {
                            for _ in &self.indent_stack {
                                self.pending
                                    .push_back(Ok((Token::Dedent, span.start..span.start))); // zero size dedent
                            }
                            self.indent_stack.clear();
                        }
                    }

                    self.at_line_start = false;
                }
            }

            self.pending.push_back(Ok((token, span)));
            break;
        }

        self.pending.pop_front()
    }
}

pub fn tokenize(source: &str) -> Result<Vec<(Token, Span)>, LexerError> {
    let lex = IndentAwareLexer::new(source);
    lex.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<Result<(Token, Span), LexerError>> {
        IndentAwareLexer::new(source).collect()
    }

    fn lex_ok(source: &str) -> Vec<Token> {
        lex(source)
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|p| p.0)
            .collect()
    }

    #[test]
    fn test_basic_tokens() {
        let tokens = lex_ok("x := 42");
        assert_eq!(
            tokens,
            vec![Token::Id, Token::Colon, Token::Eq, Token::IntegerLiteral]
        );
    }

    #[test]
    fn test_indentation() {
        let source = "if:\n    x\ny";
        let tokens = lex_ok(source);
        assert_eq!(
            tokens,
            vec![
                Token::If,
                Token::Colon,
                Token::Newline,
                Token::Indent,
                Token::Id,
                Token::Newline,
                Token::Dedent,
                Token::Id,
            ]
        );
    }

    #[test]
    fn test_nested_indentation() {
        let source = "if:\n    if:\n        x\n    y\nz";
        let tokens = lex_ok(source);
        assert_eq!(
            tokens,
            vec![
                Token::If,
                Token::Colon,
                Token::Newline,
                Token::Indent,
                Token::If,
                Token::Colon,
                Token::Newline,
                Token::Indent,
                Token::Id,
                Token::Newline,
                Token::Dedent,
                Token::Id,
                Token::Newline,
                Token::Dedent,
                Token::Id,
            ]
        );
    }

    #[test]
    fn test_inconsistent_indent() {
        let source = "if:\n \tx";
        let results = lex(source);
        assert!(
            results
                .iter()
                .any(|r| matches!(r, Err(LexerError::InconsistentIndent(_))))
        );
    }

    #[test]
    fn test_keywords() {
        let tokens = lex_ok("if then else set var true false");
        assert_eq!(
            tokens,
            vec![
                Token::If,
                Token::Then,
                Token::Else,
                Token::Set,
                Token::Var,
                Token::True,
                Token::False
            ]
        );
    }

    #[test]
    fn test_set_expression() {
        let source = "set X = 10";
        let tokens = lex_ok(source);
        assert_eq!(
            tokens,
            vec![Token::Set, Token::Id, Token::Eq, Token::IntegerLiteral]
        );
    }

    #[test]
    fn test_var_declaration() {
        let source = "var X: int = 5";
        let tokens = lex_ok(source);
        assert_eq!(
            tokens,
            vec![
                Token::Var,
                Token::Id,
                Token::Colon,
                Token::Id,
                Token::Eq,
                Token::IntegerLiteral
            ]
        );
    }

    #[test]
    fn test_operators() {
        let tokens = lex_ok("+ - * / = <> < <= > >=");
        assert_eq!(
            tokens,
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Eq,
                Token::NotEq,
                Token::Less,
                Token::LessEq,
                Token::Greater,
                Token::GreaterEq,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex_ok(r#""hello world""#);
        assert_eq!(tokens, vec![Token::StringLiteral]);
    }

    #[test]
    fn test_template_string() {
        let tokens = lex_ok(r#""hello {name}""#);
        assert_eq!(
            tokens,
            vec![Token::TemplateHead, Token::Id, Token::TemplateTail,]
        );
    }

    #[test]
    fn test_integer_literals() {
        let tokens = lex_ok("42 0xFF");
        assert_eq!(tokens, vec![Token::IntegerLiteral, Token::IntegerLiteral]);
    }

    #[test]
    fn test_float_literal() {
        let tokens = lex_ok("3.14 1.5e10");
        assert_eq!(tokens, vec![Token::FloatLiteral, Token::FloatLiteral]);
    }

    #[test]
    fn test_char_literal() {
        let tokens = lex_ok("'a' '\\n'");
        assert_eq!(tokens, vec![Token::CharLiteral, Token::CharLiteral]);
    }

    #[test]
    fn test_identifier() {
        let tokens = lex_ok("foo bar123 _private");
        assert_eq!(tokens, vec![Token::Id, Token::Id, Token::Id]);
    }

    #[test]
    fn test_punctuation() {
        let tokens = lex_ok("( ) , :");
        assert_eq!(
            tokens,
            vec![Token::LParen, Token::RParen, Token::Comma, Token::Colon]
        );
    }

    #[test]
    fn test_empty_lines() {
        let source = "x\n\n\ny";
        let tokens = lex_ok(source);
        assert_eq!(tokens, vec![Token::Id, Token::Newline, Token::Id]);
    }

    #[test]
    fn test_multiple_statements() {
        let source = "x := 1\ny := 2";
        let tokens = lex_ok(source);
        assert_eq!(
            tokens,
            vec![
                Token::Id,
                Token::Colon,
                Token::Eq,
                Token::IntegerLiteral,
                Token::Newline,
                Token::Id,
                Token::Colon,
                Token::Eq,
                Token::IntegerLiteral,
            ]
        );
    }
}
