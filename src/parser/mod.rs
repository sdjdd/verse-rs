use thiserror::Error;

use crate::{
    ast::*,
    core::{ConstId, ConstValue, PredefinedSymbols, SymbolRegistry},
    lexer::{LexerError, Span, Token},
    parser::const_pool::ConstPool,
};

mod const_pool;

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

    #[error("SyntaxError: {message} at {span:?}")]
    SyntaxError { message: String, span: Span },

    #[error("LexerError: {inner:?} at {span:?}")]
    LexerError { inner: LexerError, span: Span },
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'src> {
    source: &'src str,
    tokens: &'src [(Token, Span)],
    pos: usize,
    current_token_span: Span,
    pub symbol_table: SymbolRegistry,
    builtin_symbols: PredefinedSymbols,

    pub const_pool: ConstPool,
    pub errors: Vec<ParseError>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, tokens: &'src [(Token, Span)]) -> Self {
        let mut st = SymbolRegistry::new();
        let builtin_symbols = PredefinedSymbols::install(&mut st);

        Self {
            source,
            tokens,
            pos: 0,
            current_token_span: 0..0,
            symbol_table: st,
            builtin_symbols,
            const_pool: ConstPool::new(),
            errors: Vec::new(),
        }
    }

    fn span(&self) -> Span {
        self.current_token_span.clone()
    }

    fn slice(&self) -> &'src str {
        &self.source[self.span()]
    }

    fn peek_n(&self, n: usize) -> Token {
        self.tokens
            .get(self.pos + n)
            .map(|p| p.0)
            .unwrap_or(Token::EOF)
    }

    fn peek(&self) -> Token {
        self.peek_n(0)
    }

    fn next(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let pair = &self.tokens[self.pos];
            self.pos += 1;
            self.current_token_span = pair.1.clone();
            pair.0
        } else {
            Token::EOF
        }
    }

    fn make_expr(&mut self, span: Span, kind: impl Into<ExprKind>) -> Expression {
        Expression {
            span,
            kind: kind.into(),
        }
    }

    fn unexpected_error(&self) -> ParseError {
        ParseError::UnexpectedToken {
            token: self.tokens[self.pos].0,
            span: self.span(),
        }
    }

    fn expect(&mut self, token: Token) -> ParseResult<()> {
        let found = self.next();
        if token == found {
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                token: found,
                span: self.span(),
            })
        }
    }

    fn consume_if(&mut self, token: Token) -> bool {
        if self.peek() == token {
            self.next();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.consume_if(Token::Newline) {}
    }

    pub fn parse(&mut self) -> Program {
        self.skip_newlines();
        let mut expressions = Vec::new();
        while !matches!(self.peek(), Token::EOF) {
            match self.parse_expression() {
                Ok(expr) => expressions.push(expr),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }
        Program { expressions }
    }

    /// Discard tokens until the next statement boundary. Does not cross a
    /// `Dedent` so block structure stays aligned for the enclosing block loop.
    fn synchronize(&mut self) {
        while !matches!(self.peek(), Token::Newline | Token::Dedent | Token::EOF) {
            self.next();
        }
    }

    fn parse_expression(&mut self) -> ParseResult<Expression> {
        match self.peek() {
            Token::If => self.parse_if_expr(),
            Token::Set => self.parse_set_expr(),
            Token::Var => self.parse_var_decl_expr(),
            Token::Id if self.looks_like_function_signature() => self.parse_function_expr(),
            _ => self.parse_decl_expr(),
        }
    }

    fn parse_type_expr(&mut self) -> ParseResult<TypeExpr> {
        if self.consume_if(Token::Question) {
            let start = self.span();
            let type_expr = self.parse_type_expr()?;
            return Ok(TypeExpr {
                span: start.start..type_expr.span.end,
                kind: TypeExprKind::Option(Box::new(type_expr)),
            });
        }

        self.expect(Token::Id)?;
        let start = self.span().start;
        match self.slice() {
            "tuple" => {
                self.expect(Token::LParen)?;
                let mut args = vec![self.parse_type_expr()?];
                while self.consume_if(Token::Comma) {
                    args.push(self.parse_type_expr()?);
                }
                self.expect(Token::RParen)?;
                Ok(TypeExpr {
                    kind: TypeExprKind::Tuple(args),
                    span: start..self.span().end,
                })
            }
            "type" => Ok(TypeExpr {
                kind: TypeExprKind::Type,
                span: self.span().clone(),
            }),
            _ => {
                let symbol = self.symbol_table.intern(self.slice());
                Ok(TypeExpr {
                    kind: TypeExprKind::Named(symbol),
                    span: start..self.span().end,
                })
            }
        }
    }

    fn parse_decl_expr(&mut self) -> ParseResult<Expression> {
        let mut lhs = self.parse_compare_chain_expr()?;

        if let ExprKind::Id(id_expr) = &lhs.kind {
            if self.consume_if(Token::Colon) {
                let typ = if self.consume_if(Token::Eq) {
                    None
                } else {
                    let typ = self.parse_type_expr()?;
                    self.expect(Token::Eq)?;
                    Some(typ)
                };

                let rhs = self.parse_expression()?;
                lhs = self.make_expr(
                    lhs.span.start..rhs.span.end,
                    DeclExpr::new(id_expr.symbol, typ, rhs),
                );
            }
        }

        Ok(lhs)
    }

    fn parse_set_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::Set)?;
        let start = self.span().start;

        let target_expr = self.parse_additive_expr()?;
        let target: LValue =
            target_expr
                .try_into()
                .map_err(|e: Expression| ParseError::SyntaxError {
                    message: "Invalid set target".to_string(),
                    span: e.span,
                })?;

        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;

        Ok(self.make_expr(start..expr.span.end, SetExpr::new(target, expr)))
    }

    fn parse_var_decl_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::Var)?;
        let start = self.span().start;

        self.expect(Token::Id)?;
        let symbol = self.symbol_table.intern(self.slice());
        let name = IdExpr::new(symbol);

        self.expect(Token::Colon)?;
        let typ = self.parse_type_expr()?;

        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;

        Ok(self.make_expr(start..expr.span.end, VarDeclExpr::new(name, typ, expr)))
    }

    fn parse_compare_chain_expr(&mut self) -> ParseResult<Expression> {
        let head = self.parse_additive_expr()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek() {
                Token::Eq => CompareOp::Eq,
                Token::NotEq => CompareOp::Ne,
                Token::Greater => CompareOp::Gt,
                Token::GreaterEq => CompareOp::Ge,
                Token::Less => CompareOp::Lt,
                Token::LessEq => CompareOp::Le,
                _ => break,
            };
            self.next();
            let expr = self.parse_additive_expr()?;
            rest.push((op, expr));
        }
        Ok(if rest.is_empty() {
            head
        } else {
            let last_expr = &rest.last().unwrap().1;
            self.make_expr(
                head.span.start..last_expr.span.end,
                CompareChainExpr::new(head, rest),
            )
        })
    }

    fn parse_additive_expr(&mut self) -> ParseResult<Expression> {
        let mut lhs = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.next();
            let rhs = self.parse_multiplicative_expr()?;
            lhs = self.make_expr(lhs.span.start..rhs.span.end, BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_multiplicative_expr(&mut self) -> ParseResult<Expression> {
        let mut lhs = self.parse_call_expr()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                _ => break,
            };
            self.next();
            let rhs = self.parse_call_expr()?;
            lhs = self.make_expr(lhs.span.start..rhs.span.end, BinaryExpr::new(lhs, op, rhs));
        }
        Ok(lhs)
    }

    fn parse_if_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::If)?;
        let start = self.span().start;

        let (test, consequent, alternate) = match self.next() {
            Token::LParen => {
                let test = self.parse_expression()?;
                self.expect(Token::RParen)?;
                let (consequent, alternate) = match self.next() {
                    // if (test):
                    //     consequent
                    // else:
                    //     alternate
                    Token::Colon => {
                        self.expect(Token::Newline)?;
                        let consequent = self.parse_block_expr()?;
                        let alternate = if self.consume_if(Token::Else) {
                            self.expect(Token::Colon)?;
                            self.expect(Token::Newline)?;
                            let alternate = self.parse_block_expr()?;
                            Some(alternate)
                        } else {
                            None
                        };
                        (consequent, alternate)
                    }
                    // if (test) then consequent else alternate
                    Token::Then => {
                        let consequent = self.parse_expression()?;
                        let alternate = if self.consume_if(Token::Else) {
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
                let test = self.parse_block_expr()?;
                self.expect(Token::Then)?;
                self.expect(Token::Colon)?;
                self.expect(Token::Newline)?;
                let consequent = self.parse_block_expr()?;
                let alternate = if self.consume_if(Token::Else) {
                    self.expect(Token::Colon)?;
                    self.expect(Token::Newline)?;
                    let alternate = self.parse_block_expr()?;
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

        let end = alternate
            .as_ref()
            .map(|e| e.span.end)
            .unwrap_or(consequent.span.end);

        Ok(self.make_expr(start..end, IfExpr::new(test, consequent, alternate)))
    }

    fn parse_block_expr(&mut self) -> ParseResult<Expression> {
        self.skip_newlines();
        self.expect(Token::Indent)?;
        let start = self.span().end;
        let mut end = start;
        let mut body = Vec::new();
        loop {
            if matches!(self.peek(), Token::Dedent | Token::EOF) {
                self.next();
                break;
            }
            match self.parse_expression() {
                Ok(expr) => {
                    body.push(expr);
                    end = self.span().end;
                }
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }
        Ok(self.make_expr(start..end, BlockExpr::new(body)))
    }

    fn parse_call_expr(&mut self) -> ParseResult<Expression> {
        let pos = self.pos;
        let mut callee = self.parse_lhs_expr()?;

        if let ExprKind::Id(id_expr) = &callee.kind
            && id_expr.symbol == self.builtin_symbols.s_tuple
        {
            self.pos = pos;
            let type_expr = self.parse_type_expr()?;
            return Ok(Expression {
                span: type_expr.span.clone(),
                kind: ExprKind::Type(type_expr),
            });
        }

        while self.consume_if(Token::LParen) {
            let mut args = vec![];
            while !self.consume_if(Token::RParen) {
                args.push(self.parse_expression()?);
                self.consume_if(Token::Comma);
            }
            callee = self.make_expr(
                callee.span.start..self.span().end,
                CallExpr::new(callee, args),
            );
        }

        Ok(callee)
    }

    fn looks_like_function_signature(&self) -> bool {
        if self.peek_n(1) != Token::LParen {
            return false;
        }
        match self.peek_n(2) {
            Token::Id | Token::RParen => self.peek_n(3) == Token::Colon,
            _ => false,
        }
    }

    fn parse_function_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::Id)?;
        let start = self.span().start;
        let name = self.symbol_table.intern(self.slice());

        let mut parse_func_signature = || -> ParseResult<(Vec<FunctionParam>, TypeExpr)> {
            self.expect(Token::LParen)?;
            let mut params = vec![];
            if !self.consume_if(Token::RParen) {
                loop {
                    self.expect(Token::Id)?;
                    let param_name = self.slice();
                    let symbol = self.symbol_table.intern(param_name);
                    self.expect(Token::Colon)?;
                    params.push(FunctionParam {
                        name: symbol,
                        typ: self.parse_type_expr()?,
                    });
                    if !self.consume_if(Token::Comma) {
                        break;
                    }
                }
                self.expect(Token::RParen)?;
            }

            self.expect(Token::Colon)?;
            let return_type = self.parse_type_expr()?;
            Ok((params, return_type))
        };

        parse_func_signature().and_then(|(params, return_type)| {
            self.expect(Token::Eq)?;
            let body = if self.consume_if(Token::Newline) {
                self.parse_block_expr()?
            } else {
                self.parse_expression()?
            };
            Ok(self.make_expr(
                start..body.span.end,
                FunctionExpr::new(name, params, return_type, body),
            ))
        })
    }

    fn parse_lhs_expr(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary_expr()?;

        while self.consume_if(Token::Dot) {
            let id_expr = self.parse_identifier_expr()?;
            expr = Expression {
                span: expr.span.start..self.span().end,
                kind: ExprKind::Member(MemberExpr {
                    object: Box::new(expr),
                    property: Box::new(id_expr),
                }),
            };
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> ParseResult<Expression> {
        let expr = match self.peek() {
            Token::Id => self.parse_identifier_expr()?,
            Token::TemplateHead => self.parse_template_expression()?,
            Token::LParen => self.parse_tuple_expr()?,
            _ => self.parse_literal_expr()?,
        };
        Ok(expr)
    }

    fn parse_identifier_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::Id)?;
        let symbol = self.symbol_table.intern(self.slice());
        let expr = self.make_expr(self.span(), IdExpr::new(symbol));

        if self.consume_if(Token::LBrace) {
            let mut args = vec![];
            while !self.consume_if(Token::RBrace) {
                args.push(self.parse_expression()?);
                self.consume_if(Token::Comma);
            }
            return Ok(Expression {
                span: expr.span.start..self.span().end,
                kind: ExprKind::Construct(ConstructExpr {
                    callee: Box::new(expr),
                    args,
                }),
            });
        }

        Ok(expr)
    }

    fn parse_literal_expr(&mut self) -> ParseResult<Expression> {
        let expr = match self.peek() {
            Token::IntegerLiteral => {
                self.next();
                self.parse_integer_literal()?
            }
            Token::FloatLiteral => {
                self.next();
                self.parse_float_literal()?
            }
            Token::CharLiteral => {
                self.next();
                self.parse_char_literal()?
            }
            Token::Char32Literal => {
                self.next();
                self.parse_char32_literal()?
            }
            Token::True => {
                self.next();
                ExprKind::Logic(true)
            }
            Token::False => {
                self.next();
                ExprKind::Logic(false)
            }
            Token::StringLiteral => {
                self.next();
                let s = self.slice();
                ExprKind::String(self.escape_string_literal(&s[1..s.len() - 1]))
            }
            _ => {
                let (token, span) = self
                    .tokens
                    .get(self.pos)
                    .cloned()
                    .unwrap_or((Token::EOF, 0..0));
                return Err(ParseError::UnexpectedToken { token, span });
            }
        };
        Ok(self.make_expr(self.span(), expr))
    }

    fn parse_integer_literal(&mut self) -> ParseResult<ExprKind> {
        let mut src = self.slice();
        let mut radix = 10;
        if src.starts_with("0x") {
            src = &src[2..];
            radix = 16;
        }
        i64::from_str_radix(src, radix)
            .map(ExprKind::Integer)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid integer literal".to_string(),
                span: self.span(),
            })
    }

    fn parse_float_literal(&mut self) -> ParseResult<ExprKind> {
        let mut src = self.slice();
        if src.ends_with("f64") {
            src = &src[..src.len() - 3];
        }
        src.parse::<f64>()
            .map(ExprKind::Float)
            .map_err(|_| ParseError::InvalidToken {
                token: "Invalid float literal".to_string(),
                span: self.span(),
            })
    }

    fn parse_char_literal(&mut self) -> ParseResult<ExprKind> {
        let mut src = self.slice();
        if src.starts_with("0o") {
            return Ok(ExprKind::Char(u8::from_str_radix(&src[2..], 16).unwrap()));
        }
        src = &src[1..src.len() - 1];
        let ch = if src.starts_with('\\') {
            escape_char(src.chars().nth(1).unwrap()) as u8
        } else {
            src.bytes().next().unwrap()
        };
        Ok(ExprKind::Char(ch))
    }

    fn parse_char32_literal(&mut self) -> ParseResult<ExprKind> {
        let src = self.slice();
        let ch = if src.starts_with("0u") {
            let value = u32::from_str_radix(&src[2..], 16).unwrap();
            std::char::from_u32(value).unwrap()
        } else {
            src.chars().nth(1).unwrap()
        };
        Ok(ExprKind::Char32(ch))
    }

    fn parse_template_expression(&mut self) -> ParseResult<Expression> {
        self.expect(Token::TemplateHead)?;
        let start = self.span().start;
        let mut elements = Vec::new();
        let src = self.slice();
        elements.push(TemplateElement::Raw(
            self.escape_string_literal(&src[1..src.len() - 1]),
        ));
        loop {
            match self.peek() {
                Token::TemplateMiddle => {
                    self.next();
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
        Ok(self.make_expr(start..self.span().end, TemplateExpression::new(elements)))
    }

    fn escape_string_literal(&mut self, src: &str) -> ConstId {
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
        self.const_pool
            .intern(ConstValue::String(chars.iter().collect()))
    }

    fn parse_tuple_expr(&mut self) -> ParseResult<Expression> {
        self.expect(Token::LParen)?;
        let start = self.span().start;
        let expr = self.parse_expression()?;
        match self.next() {
            Token::Comma => {
                let mut elements = vec![expr];
                while !self.consume_if(Token::RParen) {
                    elements.push(self.parse_expression()?);
                    self.consume_if(Token::Comma);
                }
                Ok(Expression {
                    span: start..self.span().end,
                    kind: ExprKind::Tuple(TupleExpr { elements }),
                })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_program(src: &str) -> (Program, usize) {
        let tokens = tokenize(src).unwrap();
        let mut parser = Parser::new(src, &tokens);
        let program = parser.parse();
        (program, parser.errors.len())
    }

    #[test]
    fn test_valid_program() {
        let (program, errs) = parse_program("X := 1\nY := 2");
        assert_eq!(errs, 0);
        assert_eq!(program.expressions.len(), 2);
    }

    #[test]
    fn test_multiple_errors_top_level() {
        // Two broken declarations (`X :=` and `Y :=`) followed by a valid one.
        let (program, errs) = parse_program("X :=\nY :=\nZ := 1");
        assert_eq!(errs, 2);
        assert_eq!(program.expressions.len(), 1);
    }

    #[test]
    fn test_error_inside_block_recovers() {
        // A bad line inside the if-block should not abort the block or the
        // following top-level statement.
        let src = "if (true):\n    X := 1\n    Y := \n    Z := 3\nW := 4";
        let (program, errs) = parse_program(src);
        assert_eq!(errs, 1);
        assert_eq!(program.expressions.len(), 2);
    }

    #[test]
    fn test_call_statement_not_mistaken_for_function() {
        // `Print("x")` has no `:` after `)`, so it must parse as a call, not a
        // function definition.
        let (program, errs) = parse_program("Print(\"x\")");
        assert_eq!(errs, 0);
        assert!(matches!(program.expressions[0].kind, ExprKind::Call(_)));
    }

    #[test]
    fn test_function_definition_recognized() {
        // `F():void =` has a `:` after `)`, so it must parse as a function.
        let src = "F():void =\n    Print(\"x\")";
        let (program, errs) = parse_program(src);
        assert_eq!(errs, 0);
        assert!(matches!(program.expressions[0].kind, ExprKind::Func(_)));
    }

    #[test]
    fn test_function_with_typed_params() {
        let src = "Func(A: int, B: string): void =\n    Print(\"x\")";
        let (program, errs) = parse_program(src);
        assert_eq!(
            errs, 0,
            "should recognize function signature with typed params"
        );
        assert!(matches!(program.expressions[0].kind, ExprKind::Func(_)));
    }

    #[test]
    fn test_function_with_multiline_params() {
        let src = "Func(\n    A: int,\n    B: string\n): void =\n    Print(\"x\")";
        let (program, errs) = parse_program(src);
        assert_eq!(errs, 0, "should recognize multi-line function signature");
        assert!(matches!(program.expressions[0].kind, ExprKind::Func(_)));
    }

    #[test]
    fn test_unclosed_paren_recovers() {
        // `F(X` is missing the closing `)`. The parser should report one error
        // and still parse the following valid statement.
        let (program, errs) = parse_program("F(X\nY := 1");
        assert_eq!(errs, 1);
        assert_eq!(program.expressions.len(), 1);
    }
}
