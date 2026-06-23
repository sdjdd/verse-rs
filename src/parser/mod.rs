use thiserror::Error;

use crate::{
    ast::{
        AssignmentExpr, BinaryExpr, BinaryOperator, CallExpr, Expression, IdentifierExpr, IfExpr,
        LiteralExpr, TemplateElement, TemplateExpression,
    },
    lexer::{Lexer, Token},
    parser::ParseError::SyntaxError,
};

#[derive(Debug)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected token {0:?}")]
    UnexpectedToken(Token),

    #[error("Invalid token {0}")]
    InvalidToken(String),

    #[error("SyntaxError: {0}")]
    SyntaxError(String),
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'source> {
    lexer: Lexer<'source>,
    peek_token: Option<Token>,
}

impl<'source> Parser<'source> {
    pub fn new(lexer: Lexer<'source>) -> Self {
        Self {
            lexer,
            peek_token: None,
        }
    }

    fn peek(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.peek_token {
            return Ok(token);
        }
        let token = self.next()?;
        self.peek_token = Some(token);
        Ok(token)
    }

    fn next(&mut self) -> ParseResult<Token> {
        if let Some(token) = self.peek_token.take() {
            Ok(token)
        } else if let Some(token) = self.lexer.next() {
            token.map_err(|_| ParseError::InvalidToken(self.lexer.slice().to_string()))
        } else {
            Ok(Token::EOF)
        }
    }

    fn expect(&mut self, token: Token) -> ParseResult<()> {
        match self.next() {
            Ok(tk) => {
                if token == tk {
                    Ok(())
                } else {
                    Err(ParseError::UnexpectedToken(tk))
                }
            }
            Err(err) => Err(err),
        }
    }

    fn consume_if(&mut self, token: Token) -> bool {
        match self.peek() {
            Ok(tk) => {
                if tk == token {
                    self.peek_token.take();
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
        let mut lhs = self.parse_additive_expr()?;
        while self.consume_if(Token::ColonEq) {
            let target = match lhs {
                Expression::Id(expr) => expr.name,
                _ => {
                    return Err(SyntaxError(
                        "Invalid left-hand side in assignment".to_string(),
                    ));
                }
            };
            let rhs = self.parse_expression()?;
            lhs = Expression::Assign(AssignmentExpr {
                target,
                expr: Box::new(rhs),
            });
        }
        Ok(lhs)
    }

    fn parse_additive_expr(&mut self) -> ParseResult<Expression> {
        let mut lhs = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.next().unwrap(); // consume op
            let rhs = self.parse_multiplicative_expr()?;
            lhs = Expression::Binary(BinaryExpr {
                operator: op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            })
        }
        Ok(lhs)
    }

    fn parse_multiplicative_expr(&mut self) -> ParseResult<Expression> {
        let mut lhs = self.parse_call_expr()?;
        loop {
            let op = match self.peek()? {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                _ => break,
            };
            self.next().unwrap(); // consume op
            let rhs = self.parse_call_expr()?;
            lhs = Expression::Binary(BinaryExpr {
                operator: op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });
        }
        Ok(lhs)
    }

    fn parse_call_expr(&mut self) -> ParseResult<Expression> {
        let func = self.parse_primary_expr()?;
        if self.consume_if(Token::LParen) {
            let callee = match func {
                Expression::Id(expr) => expr.name,
                _ => return Err(SyntaxError("Is not a function".to_string())),
            };
            let mut arguments = Vec::new();
            while !self.consume_if(Token::RParen) {
                let expr = self.parse_additive_expr()?;
                arguments.push(expr);
                self.consume_if(Token::Comma);
            }
            Ok(Expression::Call(CallExpr { callee, arguments }))
        } else {
            Ok(func)
        }
    }

    fn parse_if_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::If)?;
        self.expect(Token::LParen)?;
        let test = self.parse_expression()?;
        self.expect(Token::RParen)?;
        self.expect(Token::Then)?;
        let consequent = self.parse_expression()?;
        let alternate = if self.consume_if(Token::Else) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };
        Ok(Expression::If(IfExpr {
            test: Box::new(test),
            consequent: Box::new(consequent),
            alternate,
        }))
    }

    fn parse_primary_expr(&mut self) -> ParseResult<Expression> {
        let expr = match self.peek()? {
            Token::Ident => Expression::Id(self.parse_identifier_expr()?),
            Token::TemplateHead => self.parse_template_expression()?,
            Token::LParen => {
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                expr
            }
            _ => self.parse_literal_expr()?,
        };
        Ok(expr)
    }

    fn parse_identifier_expr(&mut self) -> ParseResult<IdentifierExpr> {
        self.expect(Token::Ident)?;
        let name = self.lexer.slice().to_string();
        Ok(IdentifierExpr { name })
    }

    fn parse_literal_expr(&mut self) -> ParseResult<Expression> {
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
            token => return Err(ParseError::UnexpectedToken(token)),
        };
        Ok(Expression::Literal(expr))
    }

    fn parse_integer_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
        let mut radix = 10;
        if src.starts_with("0x") {
            src = &src[2..];
            radix = 16;
        }
        i64::from_str_radix(src, radix)
            .map(|v| LiteralExpr::Integer(v))
            .map_err(|_| ParseError::InvalidToken("Invalid integer literal".to_string()))
    }

    fn parse_float_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
        if src.ends_with("f64") {
            src = &src[..src.len() - 3];
        }
        src.parse::<f64>()
            .map(|v| LiteralExpr::Float(v))
            .map_err(|_| ParseError::InvalidToken("Invalid float literal".to_string()))
    }

    fn parse_char_literal(&mut self) -> ParseResult<LiteralExpr> {
        let mut src = self.lexer.slice();
        if src.starts_with("0o") {
            return Ok(LiteralExpr::Char(
                u8::from_str_radix(&src[2..], 16).unwrap(),
            ));
        }
        src = &src[1..src.len() - 1];
        let ch = if src.starts_with("\\") {
            escape_char(src.chars().nth(1).unwrap()) as u8
        } else {
            src.bytes().nth(0).unwrap()
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
        Ok(Expression::Template(TemplateExpression { elements }))
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
