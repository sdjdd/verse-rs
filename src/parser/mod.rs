use thiserror::Error;

use crate::{
    ast::*,
    lexer::{IndentAwareLexer, LexerError, Span, Token},
};

#[derive(Debug)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected token {token:?} at {span:?}")]
    UnexpectedToken { token: Token, span: Span },

    #[error("Invalid token {token} at {span:?}")]
    InvalidToken { token: String, span: Span },

    #[error("SyntaxError: {message} at {loc}")]
    SyntaxError { message: String, loc: SourceLoc },

    #[error("LexerError: {inner:?} at {span:?}")]
    LexerError { inner: LexerError, span: Span },
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'src> {
    source: &'src str,
    lexer: IndentAwareLexer<'src>,
    next_token: Option<Token>,
    next_token_span: Span,
    current_token: Option<Token>,
    current_token_span: Span,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, lexer: IndentAwareLexer<'src>) -> Self {
        Self {
            source,
            lexer,
            next_token: None,
            next_token_span: Default::default(),
            current_token: None,
            current_token_span: Default::default(),
        }
    }

    fn slice(&self) -> &'src str {
        &self.source[self.current_token_span.clone()]
    }

    fn peek(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.next_token {
            return Ok(token);
        }
        let current_token = self.current_token;
        let current_token_span = self.current_token_span.clone();
        let next_token = self.next()?;
        self.next_token = self.current_token;
        self.next_token_span = self.current_token_span.clone();
        self.current_token = current_token;
        self.current_token_span = current_token_span;
        Ok(next_token)
    }

    fn next(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.next_token.take() {
            self.current_token = Some(token);
            self.current_token_span = self.next_token_span.clone();
            return Ok(token);
        }
        loop {
            let (token, span) = match self.lexer.next() {
                Some((Ok(token), span)) => (token, span),
                Some((Err(err), span)) => {
                    return Err(ParseError::LexerError { inner: err, span });
                }
                None => (Token::EOF, 0..0),
            };

            self.current_token = Some(token);
            self.current_token_span = span;

            break Ok(token);
        }
    }

    fn make_expr(&self, start: Span, kind: impl Into<ExprKind>) -> Expression {
        Expression {
            loc: SourceLoc {
                start,
                end: self.current_token_span.clone(),
            },
            kind: kind.into(),
        }
    }

    fn unexpected_error(&self) -> ParseError {
        ParseError::UnexpectedToken {
            token: self.current_token.unwrap(),
            span: self.current_token_span.clone(),
        }
    }

    fn expect(&mut self, token: Token) -> ParseResult<()> {
        self.next().and_then(|actual| {
            if token == actual {
                Ok(())
            } else {
                Err(ParseError::UnexpectedToken {
                    token: actual,
                    span: self.current_token_span.clone(),
                })
            }
        })
    }

    fn consume_if(&mut self, token: Token) -> ParseResult<bool> {
        match self.peek()? {
            tk => {
                if tk == token {
                    self.current_token = self.next_token.take();
                    self.current_token_span = self.next_token_span.clone();
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn skip_newlines(&mut self) -> ParseResult<()> {
        while self.consume_if(Token::Newline)? {}
        Ok(())
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        self.skip_newlines()?;
        let mut expressions = Vec::new();
        while !matches!(self.peek()?, Token::EOF) {
            expressions.push(self.parse_expression()?);
            self.skip_newlines()?;
        }
        Ok(Program { expressions })
    }

    fn parse_expression(&mut self) -> ParseResult<Expression> {
        match self.peek()? {
            Token::If => self.parse_if_expr(),
            _ => self.parse_assignment_expr(),
        }
    }

    fn parse_assignment_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let mut lhs = self.parse_compare_chain_expr()?;
        while self.consume_if(Token::ColonEq)? {
            let target = match &lhs.kind {
                ExprKind::Id(expr) => expr.name.clone(),
                _ => {
                    return Err(ParseError::SyntaxError {
                        message: "Invalid left-hand side in assignment".to_string(),
                        loc: lhs.loc,
                    });
                }
            };
            let rhs = self.parse_expression()?;
            lhs = self.make_expr(start.clone(), AssignmentExpr::new(target, rhs));
        }
        Ok(lhs)
    }

    fn parse_compare_chain_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let head = self.parse_additive_expr()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek()? {
                Token::Eq => CompareOp::Eq,
                Token::NotEq => CompareOp::Ne,
                Token::Greater => CompareOp::Gt,
                Token::GreaterEq => CompareOp::Ge,
                Token::Less => CompareOp::Lt,
                Token::LessEq => CompareOp::Le,
                _ => break,
            };
            self.next().unwrap();
            let expr = self.parse_additive_expr()?;
            rest.push((op, expr));
        }
        Ok(if rest.is_empty() {
            head
        } else {
            self.make_expr(start, CompareChainExpr::new(head, rest))
        })
    }

    fn parse_additive_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let mut lhs = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.next().unwrap();
            let rhs = self.parse_multiplicative_expr()?;
            lhs = self.make_expr(start.clone(), BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_multiplicative_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let mut lhs = self.parse_call_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                _ => break,
            };
            self.next().unwrap();
            let rhs = self.parse_call_expr()?;
            lhs = self.make_expr(start.clone(), BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_call_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let callee = self.parse_primary_expr()?;
        if self.consume_if(Token::LParen)? {
            let mut args = Vec::new();
            while !self.consume_if(Token::RParen)? {
                let expr = self.parse_additive_expr()?;
                args.push(expr);
                self.consume_if(Token::Comma)?;
            }
            Ok(self.make_expr(start, CallExpr::new(callee, args)))
        } else {
            Ok(callee)
        }
    }

    fn parse_if_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        self.expect(Token::If)?;

        let (test, consequent, alternate) = match self.next()? {
            Token::LParen => {
                let test = self.parse_expression()?;
                self.expect(Token::RParen)?;
                let (consequent, alternate) = match self.next()? {
                    // if (test):
                    //     consequent
                    // else:
                    //     alternate
                    Token::Colon => {
                        self.expect(Token::Newline)?;
                        let consequent = self.parse_block()?;
                        let alternate = if self.consume_if(Token::Else)? {
                            self.expect(Token::Colon)?;
                            self.expect(Token::Newline)?;
                            let alternate = self.parse_block()?;
                            Some(alternate)
                        } else {
                            None
                        };
                        (consequent, alternate)
                    }
                    // if (test) then consequent else alternate
                    Token::Then => {
                        let consequent = self.parse_expression()?;
                        let alternate = if self.consume_if(Token::Else)? {
                            Some(self.parse_expression()?)
                        } else {
                            None
                        };
                        (consequent, alternate)
                    }
                    _ => {
                        return Err(self.unexpected_error());
                    }
                };
                (test, consequent, alternate)
            }
            // if:
            //     test
            // then:
            //     consequent
            // else:
            //     alternate
            Token::Colon => {
                self.expect(Token::Newline)?;
                let test = self.parse_block()?;
                self.expect(Token::Then)?;
                self.expect(Token::Colon)?;
                self.expect(Token::Newline)?;
                let consequent = self.parse_block()?;
                let alternate = if self.consume_if(Token::Else)? {
                    self.expect(Token::Colon)?;
                    self.expect(Token::Newline)?;
                    let alternate = self.parse_block()?;
                    Some(alternate)
                } else {
                    None
                };
                (test, consequent, alternate)
            }
            _ => {
                return Err(self.unexpected_error());
            }
        };

        Ok(self.make_expr(start, IfExpr::new(test, consequent, alternate)))
    }

    fn parse_block(&mut self) -> ParseResult<Expression> {
        self.skip_newlines()?;
        let start = self.current_token_span.clone();
        self.expect(Token::Indent)?;
        let mut body = Vec::new();
        loop {
            if matches!(self.peek()?, Token::Dedent | Token::EOF) {
                self.next().unwrap();
                break;
            }
            body.push(self.parse_expression()?);
            self.skip_newlines()?;
        }
        Ok(self.make_expr(start, BlockExpr::new(body)))
    }

    fn parse_primary_expr(&mut self) -> ParseResult<Expression> {
        let expr = match self.peek()? {
            Token::Ident => self.parse_identifier_expr()?,
            Token::TemplateHead => self.parse_template_expression()?,
            Token::LParen => self.parse_tuple_expr()?,
            _ => self.parse_literal_expr()?,
        };
        Ok(expr)
    }

    fn parse_identifier_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        self.expect(Token::Ident)?;
        let name = self.slice().to_string();
        Ok(self.make_expr(start, IdentifierExpr::new(name)))
    }

    fn parse_literal_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        let expr = match self.next()? {
            Token::IntegerLiteral => self.parse_integer_literal()?,
            Token::FloatLiteral => self.parse_float_literal()?,
            Token::CharLiteral => self.parse_char_literal()?,
            Token::Char32Literal => self.parse_char32_literal()?,
            Token::True => LiteralExpr::Bool(true),
            Token::False => LiteralExpr::Bool(false),
            Token::StringLiteral => {
                let s = self.slice();
                LiteralExpr::String(self.escape_string_literal(&s[1..s.len() - 1]))
            }
            _ => {
                return Err(self.unexpected_error());
            }
        };
        Ok(self.make_expr(start, expr))
    }

    fn parse_integer_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.slice();
        let mut radix = 10;
        if src.starts_with("0x") {
            src = &src[2..];
            radix = 16;
        }
        i64::from_str_radix(src, radix)
            .map(LiteralExpr::Integer)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid integer literal".to_string(),
                span: self.current_token_span.clone(),
            })
    }

    fn parse_float_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.slice();
        if src.ends_with("f64") {
            src = &src[..src.len() - 3];
        }
        src.parse::<f64>()
            .map(LiteralExpr::Float)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid float literal".to_string(),
                span: self.current_token_span.clone(),
            })
    }

    fn parse_char_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.slice();
        if src.starts_with("0o") {
            return Ok(LiteralExpr::Char(
                u8::from_str_radix(&src[2..], 16).unwrap(),
            ));
        }
        src = &src[1..src.len() - 1];
        let ch = if src.starts_with('\\') {
            escape_char(src.chars().nth(1).unwrap()) as u8
        } else {
            src.bytes().next().unwrap()
        };
        Ok(LiteralExpr::Char(ch))
    }

    fn parse_char32_literal(&mut self) -> ParseResult<LiteralExpr> {
        let src = self.slice();
        let ch = if src.starts_with("0u") {
            let value = u32::from_str_radix(&src[2..], 16).unwrap();
            std::char::from_u32(value).unwrap()
        } else {
            src.chars().nth(1).unwrap()
        };
        Ok(LiteralExpr::Char32(ch))
    }

    fn parse_template_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        self.expect(Token::TemplateHead)?;
        let mut elements = Vec::new();
        let src = self.slice();
        elements.push(TemplateElement::Raw(
            self.escape_string_literal(&src[1..src.len() - 1]),
        ));
        loop {
            match self.peek()? {
                Token::TemplateMiddle => {
                    self.next().unwrap();
                    let src = self.slice();
                    elements.push(TemplateElement::Raw(
                        self.escape_string_literal(&src[1..src.len() - 1]),
                    ));
                }
                Token::TemplateTail => break,
                _ => elements.push(TemplateElement::Expr(self.parse_expression()?)),
            }
        }
        self.expect(Token::TemplateTail)?;
        let src = self.slice();
        elements.push(TemplateElement::Raw(
            self.escape_string_literal(&src[1..src.len() - 1]),
        ));
        Ok(self.make_expr(start, TemplateExpression::new(elements)))
    }

    fn escape_string_literal(&self, src: &str) -> String {
        let mut chars = Vec::new();
        let mut escaped = false;
        for mut ch in src.chars() {
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if escaped {
                ch = escape_char(ch);
                escaped = false;
            }
            chars.push(ch);
        }
        chars.iter().collect()
    }

    fn parse_tuple_expr(&mut self) -> ParseResult<Expression> {
        let start = self.current_token_span.clone();
        self.expect(Token::LParen)?;
        let expr = self.parse_expression()?;
        match self.next()? {
            Token::Comma => {
                let mut elements = vec![expr];
                while !self.consume_if(Token::RParen)? {
                    elements.push(self.parse_expression()?);
                    self.consume_if(Token::Comma)?;
                }
                Ok(self.make_expr(start, TupleExpr::new(elements)))
            }
            Token::RParen => Ok(expr),
            _ => Err(self.unexpected_error()),
        }
    }
}

fn escape_char(ch: char) -> char {
    match ch {
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
        _ => ch,
    }
}
