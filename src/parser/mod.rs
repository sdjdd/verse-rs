use thiserror::Error;

use crate::{
    ast::*,
    lexer::{Lexer, Token},
    parser::ParseError::SyntaxError,
};

#[derive(Debug)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected token {token:?} at {pos}")]
    UnexpectedToken {
        token: Token,
        expected: Option<Token>,
        pos: Position,
    },

    #[error("Invalid token {token} at {pos}")]
    InvalidToken { token: String, pos: Position },

    #[error("SyntaxError: {message} at {loc}")]
    SyntaxError { message: String, loc: SourceLoc },
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'source> {
    lexer: Lexer<'source>,
    pos: Position,
    current_token: Option<Token>,
    current_token_pos: Position,
}

impl<'source> Parser<'source> {
    pub fn new(lexer: Lexer<'source>) -> Self {
        Self {
            lexer,
            pos: Position::default(),
            current_token: None,
            current_token_pos: Position::default(),
        }
    }

    fn peek(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.current_token {
            return Ok(token);
        }
        let token = self.next()?;
        self.current_token = Some(token);
        Ok(token)
    }

    fn next(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.current_token.take() {
            return Ok(token);
        }
        let token = loop {
            self.current_token_pos = self.pos;
            let token = match self.lexer.next() {
                Some(token) => token.map_err(|_| ParseError::InvalidToken {
                    token: self.lexer.slice().to_string(),
                    pos: self.pos,
                })?,
                None => break Token::EOF,
            };
            match token {
                Token::Whitespace | Token::Tabs => {
                    self.pos.col += self.lexer.slice().len();
                    continue;
                }
                Token::Newline => {
                    self.pos.ln += 1;
                    self.pos.col = 1;
                    continue;
                }
                _ => break token,
            }
        };
        self.pos.col += self.lexer.slice().len();
        Ok(token)
    }

    fn make_expr(&self, start: Position, kind: impl Into<ExprKind>) -> Expression {
        Expression {
            loc: SourceLoc {
                start,
                end: self.pos,
            },
            kind: kind.into(),
        }
    }

    fn expect(&mut self, token: Token) -> ParseResult<()> {
        match self.next() {
            Ok(tk) => {
                if token == tk {
                    Ok(())
                } else {
                    Err(ParseError::UnexpectedToken {
                        token: tk,
                        expected: Some(token),
                        pos: self.pos,
                    })
                }
            }
            Err(err) => Err(err),
        }
    }

    fn consume_if(&mut self, token: Token) -> bool {
        match self.peek() {
            Ok(tk) => {
                if tk == token {
                    self.current_token.take();
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        let mut expressions = Vec::new();
        while !matches!(self.peek()?, Token::EOF) {
            expressions.push(self.parse_expression()?);
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
        let start = self.pos;
        let mut lhs = self.parse_compare_chain_expr()?;
        while self.consume_if(Token::ColonEq) {
            let target = match &lhs.kind {
                ExprKind::Id(expr) => expr.name.clone(),
                _ => {
                    return Err(SyntaxError {
                        message: "Invalid left-hand side in assignment".to_string(),
                        loc: lhs.loc,
                    });
                }
            };
            let rhs = self.parse_expression()?;
            lhs = self.make_expr(start, AssignmentExpr::new(target, rhs));
        }
        Ok(lhs)
    }

    fn parse_compare_chain_expr(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
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
        let start = self.pos;
        let mut lhs = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.next().unwrap();
            let rhs = self.parse_multiplicative_expr()?;
            lhs = self.make_expr(start, BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_multiplicative_expr(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
        let mut lhs = self.parse_call_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                _ => break,
            };
            self.next().unwrap();
            let rhs = self.parse_call_expr()?;
            lhs = self.make_expr(start, BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_call_expr(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
        let callee = self.parse_primary_expr()?;
        if self.consume_if(Token::LParen) {
            let mut args = Vec::new();
            while !self.consume_if(Token::RParen) {
                let expr = self.parse_additive_expr()?;
                args.push(expr);
                self.consume_if(Token::Comma);
            }
            Ok(self.make_expr(start, CallExpr::new(callee, args)))
        } else {
            Ok(callee)
        }
    }

    fn parse_if_expr(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
        self.expect(Token::If)?;
        self.expect(Token::LParen)?;
        let test = self.parse_expression()?;
        self.expect(Token::RParen)?;
        self.expect(Token::Then)?;
        let consequent = self.parse_expression()?;
        let alternate = if self.consume_if(Token::Else) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(self.make_expr(start, IfExpr::new(test, consequent, alternate)))
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
        let start = self.pos;
        self.expect(Token::Ident)?;
        let name = self.lexer.slice().to_string();
        Ok(self.make_expr(start, IdentifierExpr::new(name)))
    }

    fn parse_literal_expr(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
        let expr = match self.next()? {
            Token::IntegerLiteral => self.parse_integer_literal()?,
            Token::FloatLiteral => self.parse_float_literal()?,
            Token::CharLiteral => self.parse_char_literal()?,
            Token::Char32Literal => self.parse_char32_literal()?,
            Token::True => LiteralExpr::Bool(true),
            Token::False => LiteralExpr::Bool(false),
            Token::StringLiteral => LiteralExpr::String(
                self.escape_string_literal(&self.lexer.slice()[1..self.lexer.slice().len() - 1]),
            ),
            token => {
                return Err(ParseError::UnexpectedToken {
                    token,
                    expected: None,
                    pos: self.current_token_pos,
                });
            }
        };
        Ok(self.make_expr(start, expr))
    }

    fn parse_integer_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
        let mut radix = 10;
        if src.starts_with("0x") {
            src = &src[2..];
            radix = 16;
        }
        i64::from_str_radix(src, radix)
            .map(LiteralExpr::Integer)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid integer literal".to_string(),
                pos: self.current_token_pos,
            })
    }

    fn parse_float_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
        if src.ends_with("f64") {
            src = &src[..src.len() - 3];
        }
        src.parse::<f64>()
            .map(LiteralExpr::Float)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid float literal".to_string(),
                pos: self.current_token_pos,
            })
    }

    fn parse_char_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
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
        let src = self.lexer.slice();
        let ch = if src.starts_with("0u") {
            let value = u32::from_str_radix(&src[2..], 16).unwrap();
            std::char::from_u32(value).unwrap()
        } else {
            src.chars().nth(1).unwrap()
        };
        Ok(LiteralExpr::Char32(ch))
    }

    fn parse_template_expression(&mut self) -> ParseResult<Expression> {
        let start = self.pos;
        self.expect(Token::TemplateHead)?;
        let mut elements = Vec::new();
        let src = self.lexer.slice();
        elements.push(TemplateElement::Raw(
            self.escape_string_literal(&src[1..src.len() - 1]),
        ));
        loop {
            match self.peek()? {
                Token::TemplateMiddle => {
                    self.next().unwrap();
                    let src = self.lexer.slice();
                    elements.push(TemplateElement::Raw(
                        self.escape_string_literal(&src[1..src.len() - 1]),
                    ));
                }
                Token::TemplateTail => break,
                _ => elements.push(TemplateElement::Expr(self.parse_expression()?)),
            }
        }
        self.expect(Token::TemplateTail)?;
        let src = self.lexer.slice();
        elements.push(TemplateElement::Raw(
            self.escape_string_literal(&src[1..src.len() - 1]),
        ));
        Ok(self.make_expr(start, TemplateExpression::new(elements)))
    }

    fn escape_string_literal(&mut self, src: &str) -> String {
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
        let start = self.pos;
        self.expect(Token::LParen)?;
        let expr = self.parse_expression()?;
        match self.next()? {
            Token::Comma => {
                let mut elements = vec![expr];
                while !self.consume_if(Token::RParen) {
                    elements.push(self.parse_expression()?);
                    self.consume_if(Token::Comma);
                }
                Ok(self.make_expr(start, TupleExpr::new(elements)))
            }
            Token::RParen => Ok(expr),
            token => Err(ParseError::UnexpectedToken {
                token,
                expected: None,
                pos: self.current_token_pos,
            }),
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
