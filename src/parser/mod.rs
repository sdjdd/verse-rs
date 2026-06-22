use thiserror::Error;

use crate::{
    ast::{
        AssignmentExpr, BinaryExpr, BinaryOperator, CallExpr, Expression, IdentifierExpr, IfExpr,
        LiteralExpr,
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

struct ParserState<'source> {
    lexer: Lexer<'source>,
    peek_token: Option<Token>,
}

pub struct Parser<'source> {
    lexer: Lexer<'source>,
    peek_token: Option<Token>,
    states: Vec<ParserState<'source>>,
}

impl<'source> Parser<'source> {
    pub fn new(lexer: Lexer<'source>) -> Self {
        Self {
            lexer,
            peek_token: None,
            states: vec![],
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

    fn push_state(&mut self) {
        self.states.push(ParserState {
            lexer: self.lexer.clone(),
            peek_token: self.peek_token,
        });
    }

    fn pop_state(&mut self) {
        if let Some(state) = self.states.pop() {
            self.lexer = state.lexer;
            self.peek_token = state.peek_token;
        }
    }

    fn drop_state(&mut self) {
        self.states.pop();
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        let mut expressions = Vec::new();
        while !matches!(self.peek(), Ok(Token::EOF)) {
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
        if self.consume_if(Token::LParen) {
            let expr = self.parse_expression()?;
            self.expect(Token::RParen)?;
            return Ok(expr);
        }

        self.push_state();
        if let Ok(expr) = self.parse_identifier_expr() {
            self.drop_state();
            return Ok(Expression::Id(expr));
        }
        self.pop_state();

        Ok(Expression::Literal(self.parse_literal_expr()?))
    }

    fn parse_identifier_expr(&mut self) -> ParseResult<IdentifierExpr> {
        self.expect(Token::Ident)?;
        let name = self.lexer.slice().to_string();
        Ok(IdentifierExpr { name })
    }

    fn parse_literal_expr(&mut self) -> ParseResult<LiteralExpr> {
        let expr = match self.next()? {
            Token::Integer => {
                let src = self.lexer.slice();
                let value = if src.len() > 2 {
                    // TODO: check overflow
                    match &src[0..2] {
                        "0x" => i64::from_str_radix(&src[2..], 16).unwrap(),
                        _ => i64::from_str_radix(src, 10).unwrap(),
                    }
                } else {
                    src.parse::<i64>().unwrap()
                };
                LiteralExpr::Integer(value)
            }
            Token::FloatLiteral => {
                let mut src = self.lexer.slice();
                if src.ends_with("f64") {
                    src = &src[..src.len() - 3]
                }
                LiteralExpr::Float(src.parse().unwrap())
            }
            Token::CharLiteral(c) => LiteralExpr::Char(c),
            Token::Char32Literal(c) => LiteralExpr::Char32(c),
            Token::True => LiteralExpr::Bool(true),
            Token::False => LiteralExpr::Bool(false),
            token => return Err(ParseError::UnexpectedToken(token)),
        };
        Ok(expr)
    }
}
