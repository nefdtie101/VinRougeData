use crate::dsl::ast::*;
use crate::dsl::error::{ParseError, ParseResult};
use crate::dsl::token::Token;

impl super::Parser {
    pub(super) fn parse_assert(&mut self) -> ParseResult<Expr> {
        self.advance();

        let label = if let Token::StringLit(s) = self.peek().clone() {
            self.advance();
            Some(s)
        } else {
            None
        };

        let lhs = self.parse_or()?;

        let op = match self.peek() {
            Token::Eq    => Some(CmpOp::Eq),
            Token::NotEq => Some(CmpOp::NotEq),
            Token::Gt    => Some(CmpOp::Gt),
            Token::Gte   => Some(CmpOp::Gte),
            Token::Lt    => Some(CmpOp::Lt),
            Token::Lte   => Some(CmpOp::Lte),
            _            => None,
        };

        if let Some(op) = op {
            self.advance();
            let rhs = self.parse_or()?;
            return Ok(Expr::Assert { label, lhs: Box::new(lhs), rhs: Box::new(rhs), op });
        }

        Ok(Expr::Assert { label, lhs: Box::new(lhs), rhs: Box::new(Expr::Bool(true)), op: CmpOp::Eq })
    }

    pub(super) fn parse_sample(&mut self) -> ParseResult<Expr> {
        self.advance();

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

        self.eat(&Token::From);

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

        self.eat(&Token::Size);

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

    pub(super) fn parse_relation(&mut self) -> ParseResult<Expr> {
        let pos = self.peek_pos();
        self.advance();

        let from = match self.peek().clone() {
            Token::Ident(s) => { self.advance(); s }
            other => return Err(ParseError::new(pos, format!("expected table.column after RELATION, got {other}"))),
        };
        let dot = from.find('.').ok_or_else(|| {
            ParseError::new(pos, format!("RELATION source must be table.column, got '{from}'"))
        })?;
        let from_table = from[..dot].to_string();
        let from_col   = from[dot + 1..].to_string();

        let arrow_pos = self.peek_pos();
        if self.peek() != &Token::Arrow {
            return Err(ParseError::new(arrow_pos, format!("expected '->' in RELATION, got {}", self.peek())));
        }
        self.advance();

        let to = match self.peek().clone() {
            Token::Ident(s) => { self.advance(); s }
            other => return Err(ParseError::new(self.peek_pos(), format!("expected table.column after '->', got {other}"))),
        };
        let dot = to.find('.').ok_or_else(|| {
            ParseError::new(pos, format!("RELATION target must be table.column, got '{to}'"))
        })?;
        let to_table  = to[..dot].to_string();
        let to_col    = to[dot + 1..].to_string();

        Ok(Expr::RelationDecl { from_table, from_col, to_table, to_col })
    }

    pub(super) fn parse_chart(&mut self) -> ParseResult<Expr> {
        self.advance(); // consume CHART

        let chart_type = match self.peek().clone() {
            Token::Ident(s) => { self.advance(); s }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected chart type identifier after CHART, got {other}"),
            )),
        };

        let aggregate = Box::new(self.parse_or()?);

        if self.peek() != &Token::By {
            return Err(ParseError::new(
                self.peek_pos(),
                format!("expected BY after chart aggregate, got {}", self.peek()),
            ));
        }
        self.advance(); // consume BY

        let dimension = Box::new(self.parse_or()?);

        Ok(Expr::Chart { chart_type, aggregate, dimension })
    }

    pub(super) fn parse_screen(&mut self) -> ParseResult<Expr> {
        self.advance(); // consume SCREEN

        let title = match self.peek().clone() {
            Token::StringLit(s) => { self.advance(); s }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected title string after SCREEN, got {other}"),
            )),
        };

        if self.peek() != &Token::LBrace {
            return Err(ParseError::new(
                self.peek_pos(),
                format!("expected '{{' after SCREEN title, got {}", self.peek()),
            ));
        }
        self.advance(); // consume {

        let mut charts = Vec::new();
        while self.peek() != &Token::RBrace && self.peek() != &Token::Eof {
            // Optional label inside screen block
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

            if self.peek() != &Token::Chart {
                return Err(ParseError::new(
                    self.peek_pos(),
                    format!("expected CHART inside SCREEN block, got {}", self.peek()),
                ));
            }
            self.advance(); // consume CHART

            let chart_type = match self.peek().clone() {
                Token::Ident(s) => { self.advance(); s }
                other => return Err(ParseError::new(
                    self.peek_pos(),
                    format!("expected chart type identifier after CHART, got {other}"),
                )),
            };

            let aggregate = Box::new(self.parse_or()?);

            if self.peek() != &Token::By {
                return Err(ParseError::new(
                    self.peek_pos(),
                    format!("expected BY after chart aggregate, got {}", self.peek()),
                ));
            }
            self.advance(); // consume BY

            let dimension = Box::new(self.parse_or()?);

            charts.push(ChartDef { label, chart_type, aggregate, dimension });
        }

        if self.peek() != &Token::RBrace {
            return Err(ParseError::new(
                self.peek_pos(),
                "expected '}' to close SCREEN block".to_string(),
            ));
        }
        self.advance(); // consume }

        Ok(Expr::Screen { title, charts })
    }

    pub(super) fn parse_section(&mut self) -> ParseResult<Expr> {
        self.advance(); // consume SECTION

        let title = match self.peek().clone() {
            Token::StringLit(s) => { self.advance(); s }
            other => return Err(ParseError::new(
                self.peek_pos(),
                format!("expected title string after SECTION, got {other}"),
            )),
        };

        if self.peek() != &Token::LBrace {
            return Err(ParseError::new(
                self.peek_pos(),
                format!("expected '{{' after SECTION title, got {}", self.peek()),
            ));
        }
        self.advance(); // consume {

        let mut statements = Vec::new();
        while self.peek() != &Token::RBrace && self.peek() != &Token::Eof {
            statements.push(self.parse_statement()?);
        }

        if self.peek() != &Token::RBrace {
            return Err(ParseError::new(
                self.peek_pos(),
                "expected '}' to close SECTION block".to_string(),
            ));
        }
        self.advance(); // consume }

        Ok(Expr::Section { title, statements })
    }
}
