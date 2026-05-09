use crate::dsl::ast::{self, *};
use crate::dsl::error::{ParseError, ParseResult};
use crate::dsl::token::Token;
use rust_decimal::Decimal;

impl super::Parser {
    pub(super) fn parse_expr(&mut self) -> ParseResult<Expr> {
        if self.peek() == &Token::Assert {
            return self.parse_assert();
        }
        if self.peek() == &Token::Sample {
            return self.parse_sample();
        }
        if self.peek() == &Token::Relation {
            return self.parse_relation();
        }
        if self.peek() == &Token::Chart {
            return self.parse_chart();
        }
        if self.peek() == &Token::Section {
            return self.parse_section();
        }
        if self.peek() == &Token::Schema {
            self.advance();
            return Ok(Expr::Schema);
        }
        self.parse_or()
    }

    pub(super) fn parse_or(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.peek() == &Token::Or {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::Logical {
                op: LogicOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    pub(super) fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_not()?;
        while self.peek() == &Token::And {
            self.advance();
            let rhs = self.parse_not()?;
            lhs = Expr::Logical {
                op: LogicOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    pub(super) fn parse_not(&mut self) -> ParseResult<Expr> {
        if self.peek() == &Token::Not {
            self.advance();
            let expr = self.parse_cmp()?;
            return Ok(Expr::Not(Box::new(expr)));
        }
        self.parse_cmp()
    }

    pub(super) fn parse_cmp(&mut self) -> ParseResult<Expr> {
        let lhs = self.parse_add()?;

        if self.peek() == &Token::Is {
            self.advance();
            let negated = self.eat(&Token::Not);
            self.expect(&Token::Null)?;
            return Ok(Expr::IsNull {
                expr: Box::new(lhs),
                negated,
            });
        }

        let negated = if self.peek() == &Token::Not {
            self.advance();
            true
        } else {
            false
        };

        if self.peek() == &Token::Like {
            self.advance();
            let pattern = self.parse_add()?;
            return Ok(Expr::Like {
                expr: Box::new(lhs),
                pattern: Box::new(pattern),
                negated,
            });
        }

        if self.peek() == &Token::In {
            self.advance();
            if let Token::Ident(ref col_ref) = self.peek().clone() {
                if col_ref.contains('.') {
                    let col_ref = col_ref.clone();
                    self.advance();
                    let dot = col_ref.find('.').unwrap();
                    let table = col_ref[..dot].to_string();
                    let column = col_ref[dot + 1..].to_string();
                    return Ok(Expr::InTableCol {
                        expr: Box::new(lhs),
                        table,
                        column,
                        negated,
                    });
                }
            }
            self.expect(&Token::LParen)?;
            let mut values = vec![self.parse_add()?];
            while self.eat(&Token::Comma) {
                values.push(self.parse_add()?);
            }
            self.expect(&Token::RParen)?;
            return Ok(Expr::InList {
                expr: Box::new(lhs),
                values,
                negated,
            });
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
            return Err(ParseError::new(
                self.peek_pos(),
                "expected LIKE, IN, or BETWEEN after NOT",
            ));
        }

        let op = match self.peek() {
            Token::Eq => CmpOp::Eq,
            Token::NotEq => CmpOp::NotEq,
            Token::Gt => CmpOp::Gt,
            Token::Gte => CmpOp::Gte,
            Token::Lt => CmpOp::Lt,
            Token::Lte => CmpOp::Lte,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::Compare {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }

    pub(super) fn parse_add(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => ArithOp::Add,
                Token::Minus => ArithOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    pub(super) fn parse_mul(&mut self) -> ParseResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => ArithOp::Mul,
                Token::Slash => ArithOp::Div,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    pub(super) fn parse_unary(&mut self) -> ParseResult<Expr> {
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

    pub(super) fn parse_primary(&mut self) -> ParseResult<Expr> {
        let pos = self.peek_pos();
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                let _ = self.eat(&Token::Percent);
                Ok(Expr::Number(n))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Null)
            }

            Token::Sum => self.parse_aggregate(AggFunc::Sum),
            Token::Avg => self.parse_aggregate(AggFunc::Avg),
            Token::Count => self.parse_aggregate(AggFunc::Count),
            Token::Min => self.parse_aggregate(AggFunc::Min),
            Token::Max => self.parse_aggregate(AggFunc::Max),

            Token::Upper => self.parse_string_fn(ast::StringFunc::Upper),
            Token::Lower => self.parse_string_fn(ast::StringFunc::Lower),
            Token::Trim => self.parse_string_fn(ast::StringFunc::Trim),
            Token::Length => self.parse_string_fn(ast::StringFunc::Length),
            Token::SubStr => self.parse_substr(),
            Token::Concat => self.parse_concat(),
            Token::Date => self.parse_date_fn(),
            Token::Case => self.parse_case(),

            Token::Distinct => {
                self.advance();
                self.parse_primary()
            }

            Token::Coalesce => self.parse_coalesce(),
            Token::NullIf => self.parse_nullif(),
            Token::Iif => self.parse_iif(),
            Token::Abs => self.parse_math_fn(ast::MathFunc::Abs),
            Token::Round => self.parse_math_fn(ast::MathFunc::Round),
            Token::CountIf => self.parse_countif(),
            Token::SumIf => self.parse_sumif(),
            Token::IsBlank => self.parse_pred_fn(|expr| Expr::IsBlank {
                expr,
                negated: false,
            }),
            Token::IsNumeric => self.parse_pred_fn(|expr| Expr::IsNumeric {
                expr,
                negated: false,
            }),
            Token::IsDate => self.parse_pred_fn(|expr| Expr::IsDate {
                expr,
                negated: false,
            }),
            Token::Duplicated => self.parse_duplicated(),
            Token::SaIdValid => self.parse_pred_fn(|expr| Expr::SaIdValid { expr }),

            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            Token::Ident(name) => {
                if self.peek_next() == &Token::LParen {
                    return Err(ParseError::new(
                        pos,
                        format!(
                            "unknown function '{name}' — supported: \
                         SUM COUNT AVG MIN MAX COUNTIF SUMIF  \
                         UPPER LOWER TRIM LENGTH SUBSTR CONCAT DATE  \
                         COALESCE NULLIF IIF ABS ROUND CASE  \
                         IS_BLANK IS_NUMERIC IS_DATE DUPLICATED SA_ID_VALID"
                        ),
                    ));
                }
                self.advance();
                Ok(Expr::ColumnRef(name))
            }

            other => Err(ParseError::new(pos, format!("unexpected token {other}"))),
        }
    }
}
