use rust_decimal::Decimal;

use super::ast::*;
use super::error::{ParseError, ParseResult};
use super::token::Token;

pub struct Parser {
    tokens: Vec<(usize, Token)>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(usize, Token)>) -> Self {
        Self { tokens, pos: 0 }
    }

    // ── token navigation ──────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].1
    }

    fn peek_pos(&self) -> usize {
        self.tokens[self.pos].0
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos].1;
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> ParseResult<()> {
        let pos = self.peek_pos();
        let got = self.peek().clone();
        if &got == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::new(pos, format!("expected {expected}, got {got}")))
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── public entry point ────────────────────

    /// Parse a complete script (one or more statements).
    pub fn parse_script(&mut self) -> ParseResult<Vec<Statement>> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    /// Parse a single statement, optionally prefixed with `label:`.
    pub fn parse_statement(&mut self) -> ParseResult<Statement> {
        let label = if matches!(self.peek(), Token::Ident(_)) {
            if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].1 == Token::Colon {
                let name = match self.advance().clone() {
                    Token::Ident(s) => s,
                    _ => unreachable!(),
                };
                self.advance(); // consume ':'
                Some(name)
            } else {
                None
            }
        } else {
            None
        };

        let expr = self.parse_expr()?;
        Ok(Statement { label, expr })
    }

    // ── expression hierarchy ──────────────────
    //
    //  parse_expr        (OR)
    //    parse_and       (AND)
    //      parse_not     (NOT)
    //        parse_cmp   (= <> < > <= >=, IS NULL, IN, BETWEEN)
    //          parse_add (+ -)
    //            parse_mul (* /)
    //              parse_unary (unary -)
    //                parse_primary

    fn parse_expr(&mut self) -> ParseResult<Expr> {
        if self.peek() == &Token::Assert {
            return self.parse_assert();
        }
        if self.peek() == &Token::Sample {
            return self.parse_sample();
        }
        self.parse_or()
    }

    fn parse_or(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.peek() == &Token::Or {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::Logical { op: LogicOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_not()?;
        while self.peek() == &Token::And {
            self.advance();
            let rhs = self.parse_not()?;
            lhs = Expr::Logical { op: LogicOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_not(&mut self) -> ParseResult<Expr> {
        if self.peek() == &Token::Not {
            self.advance();
            let expr = self.parse_cmp()?;
            return Ok(Expr::Not(Box::new(expr)));
        }
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> ParseResult<Expr> {
        let lhs = self.parse_add()?;

        // IS NULL / IS NOT NULL
        if self.peek() == &Token::Is {
            self.advance();
            let negated = self.eat(&Token::Not);
            self.expect(&Token::Null)?;
            return Ok(Expr::IsNull { expr: Box::new(lhs), negated });
        }

        // NOT IN / NOT BETWEEN (after the lhs)
        let negated = if self.peek() == &Token::Not {
            self.advance();
            true
        } else {
            false
        };

        if self.peek() == &Token::In {
            self.advance();
            self.expect(&Token::LParen)?;
            let mut values = vec![self.parse_add()?];
            while self.eat(&Token::Comma) {
                values.push(self.parse_add()?);
            }
            self.expect(&Token::RParen)?;
            return Ok(Expr::InList { expr: Box::new(lhs), values, negated });
        }

        if self.peek() == &Token::Between {
            self.advance();
            let low = self.parse_add()?;
            self.expect(&Token::And)?;
            let high = self.parse_add()?;
            return Ok(Expr::Between {
                expr: Box::new(lhs),
                low: Box::new(low),
                high: Box::new(high),
                negated,
            });
        }

        if negated {
            return Err(ParseError::new(self.peek_pos(), "expected IN or BETWEEN after NOT"));
        }

        let op = match self.peek() {
            Token::Eq    => CmpOp::Eq,
            Token::NotEq => CmpOp::NotEq,
            Token::Gt    => CmpOp::Gt,
            Token::Gte   => CmpOp::Gte,
            Token::Lt    => CmpOp::Lt,
            Token::Lte   => CmpOp::Lte,
            _            => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::Compare { op, lhs: Box::new(lhs), rhs: Box::new(rhs) })
    }

    fn parse_add(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus  => ArithOp::Add,
                Token::Minus => ArithOp::Sub,
                _            => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star  => ArithOp::Mul,
                Token::Slash => ArithOp::Div,
                _            => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        if self.eat(&Token::Minus) {
            let expr = self.parse_primary()?;
            return match expr {
                Expr::Number(n) => Ok(Expr::Number(-n)),
                other => Ok(Expr::BinOp {
                    op: ArithOp::Sub,
                    lhs: Box::new(Expr::Number(Decimal::ZERO)),
                    rhs: Box::new(other),
                }),
            };
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let pos = self.peek_pos();
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                let _ = self.eat(&Token::Percent); // consumed by parse_sample if needed
                Ok(Expr::Number(n))
            }
            Token::StringLit(s) => { self.advance(); Ok(Expr::Str(s)) }
            Token::True         => { self.advance(); Ok(Expr::Bool(true)) }
            Token::False        => { self.advance(); Ok(Expr::Bool(false)) }
            Token::Null         => { self.advance(); Ok(Expr::Null) }

            Token::Sum   => self.parse_aggregate(AggFunc::Sum),
            Token::Avg   => self.parse_aggregate(AggFunc::Avg),
            Token::Count => self.parse_aggregate(AggFunc::Count),
            Token::Min   => self.parse_aggregate(AggFunc::Min),
            Token::Max   => self.parse_aggregate(AggFunc::Max),

            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            Token::Ident(name) => { self.advance(); Ok(Expr::ColumnRef(name)) }

            other => Err(ParseError::new(pos, format!("unexpected token {other}"))),
        }
    }

    // ── aggregate ─────────────────────────────

    fn parse_aggregate(&mut self, func: AggFunc) -> ParseResult<Expr> {
        self.advance(); // consume function name
        self.expect(&Token::LParen)?;
        let expr = self.parse_add()?;
        self.expect(&Token::RParen)?;

        let filter = if self.eat(&Token::Where) {
            Some(Box::new(self.parse_or()?))
        } else {
            None
        };

        Ok(Expr::Aggregate { func, expr: Box::new(expr), filter })
    }

    // ── assert ────────────────────────────────

    fn parse_assert(&mut self) -> ParseResult<Expr> {
        self.advance(); // consume ASSERT

        let label = if let Token::StringLit(s) = self.peek().clone() {
            self.advance();
            Some(s)
        } else {
            None
        };

        let lhs = self.parse_add()?;

        let op = match self.peek() {
            Token::Eq    => CmpOp::Eq,
            Token::NotEq => CmpOp::NotEq,
            Token::Gt    => CmpOp::Gt,
            Token::Gte   => CmpOp::Gte,
            Token::Lt    => CmpOp::Lt,
            Token::Lte   => CmpOp::Lte,
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected comparison operator in ASSERT, got {other}"),
            )),
        };
        self.advance();

        let rhs = self.parse_add()?;
        Ok(Expr::Assert { label, lhs: Box::new(lhs), rhs: Box::new(rhs), op })
    }

    // ── sample ────────────────────────────────

    fn parse_sample(&mut self) -> ParseResult<Expr> {
        self.advance(); // consume SAMPLE

        let method = match self.peek() {
            Token::Mus        => { self.advance(); SampleMethod::Mus }
            Token::Random     => { self.advance(); SampleMethod::Random }
            Token::Systematic => { self.advance(); SampleMethod::Systematic }
            Token::Stratified => { self.advance(); SampleMethod::Stratified }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected sampling method (MUS/RANDOM/SYSTEMATIC/STRATIFIED), got {other}"),
            )),
        };

        let value_column = match self.peek().clone() {
            Token::Ident(s) => { self.advance(); s }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected column reference after sampling method, got {other}"),
            )),
        };

        let population = if let Some(dot_pos) = value_column.rfind('.') {
            value_column[..dot_pos].to_string()
        } else {
            value_column.clone()
        };

        let size = match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                if self.eat(&Token::Percent) {
                    SampleSize::Percent(n)
                } else {
                    SampleSize::Count(n)
                }
            }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected sample size (number or percentage), got {other}"),
            )),
        };

        let filter = if self.eat(&Token::Where) {
            Some(Box::new(self.parse_or()?))
        } else {
            None
        };

        Ok(Expr::Sample { method, population, value_column, size, filter })
    }
}
