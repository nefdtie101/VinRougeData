use crate::dsl::ast::{self, *};
use crate::dsl::error::ParseResult;
use crate::dsl::token::Token;

impl super::Parser {
    pub(super) fn parse_coalesce(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let mut exprs = vec![Box::new(self.parse_expr()?)];
        while self.eat(&Token::Comma) {
            exprs.push(Box::new(self.parse_expr()?));
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::Coalesce { exprs })
    }

    pub(super) fn parse_nullif(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let compare = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Expr::NullIf {
            expr: Box::new(expr),
            compare: Box::new(compare),
        })
    }

    pub(super) fn parse_iif(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let condition = self.parse_or()?;
        self.expect(&Token::Comma)?;
        let then_expr = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let else_expr = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Expr::Case {
            branches: vec![(Box::new(condition), Box::new(then_expr))],
            else_expr: Some(Box::new(else_expr)),
        })
    }

    pub(super) fn parse_math_fn(&mut self, func: MathFunc) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        let scale = if self.eat(&Token::Comma) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect(&Token::RParen)?;
        Ok(Expr::MathFn {
            func,
            expr: Box::new(expr),
            scale,
        })
    }

    pub(super) fn parse_countif(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let col = self.parse_expr()?;
        let filter = if self.eat(&Token::Comma) {
            let criteria = self.parse_expr()?;
            Some(Box::new(Expr::Compare {
                op: CmpOp::Eq,
                lhs: Box::new(col.clone()),
                rhs: Box::new(criteria),
            }))
        } else {
            None
        };
        self.expect(&Token::RParen)?;
        Ok(Expr::Aggregate {
            func: AggFunc::Count,
            distinct: false,
            expr: Box::new(col),
            filter,
        })
    }

    pub(super) fn parse_sumif(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let range_col = self.parse_expr()?;
        let (criteria, sum_col) = if self.eat(&Token::Comma) {
            let crit = self.parse_expr()?;
            let sum = if self.eat(&Token::Comma) {
                self.parse_expr()?
            } else {
                range_col.clone()
            };
            (Some(crit), sum)
        } else {
            (None, range_col.clone())
        };
        self.expect(&Token::RParen)?;
        let filter = criteria.map(|c| {
            Box::new(Expr::Compare {
                op: CmpOp::Eq,
                lhs: Box::new(range_col),
                rhs: Box::new(c),
            })
        });
        Ok(Expr::Aggregate {
            func: AggFunc::Sum,
            distinct: false,
            expr: Box::new(sum_col),
            filter,
        })
    }

    pub(super) fn parse_string_fn(&mut self, func: ast::StringFunc) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Expr::StringFn {
            func,
            expr: Box::new(expr),
        })
    }

    pub(super) fn parse_substr(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let start = self.parse_expr()?;
        let length = if self.eat(&Token::Comma) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect(&Token::RParen)?;
        Ok(Expr::SubStr {
            expr: Box::new(expr),
            start: Box::new(start),
            length,
        })
    }

    pub(super) fn parse_concat(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let mut exprs = vec![Box::new(self.parse_expr()?)];
        while self.eat(&Token::Comma) {
            exprs.push(Box::new(self.parse_expr()?));
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::Concat { exprs })
    }

    pub(super) fn parse_date_fn(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Expr::DateFn {
            expr: Box::new(expr),
        })
    }

    pub(super) fn parse_case(&mut self) -> ParseResult<Expr> {
        use crate::dsl::error::ParseError;
        self.advance();
        let mut branches = Vec::new();
        while self.peek() == &Token::When {
            self.advance();
            let condition = self.parse_or()?;
            self.expect(&Token::Then)?;
            let result = self.parse_or()?;
            branches.push((Box::new(condition), Box::new(result)));
        }
        if branches.is_empty() {
            return Err(ParseError::new(
                self.peek_pos(),
                "CASE must have at least one WHEN branch",
            ));
        }
        let else_expr = if self.eat(&Token::Else) {
            Some(Box::new(self.parse_or()?))
        } else {
            None
        };
        self.expect(&Token::End)?;
        Ok(Expr::Case {
            branches,
            else_expr,
        })
    }

    pub(super) fn parse_aggregate(&mut self, func: AggFunc) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let distinct = self.eat(&Token::Distinct);
        let expr = self.parse_add()?;
        self.expect(&Token::RParen)?;

        let filter = if self.eat(&Token::Where) {
            Some(Box::new(self.parse_or()?))
        } else {
            None
        };

        Ok(Expr::Aggregate {
            func,
            distinct,
            expr: Box::new(expr),
            filter,
        })
    }

    pub(super) fn parse_pred_fn<F>(&mut self, build: F) -> ParseResult<Expr>
    where
        F: FnOnce(Box<Expr>) -> Expr,
    {
        self.advance();
        self.expect(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(build(Box::new(expr)))
    }

    pub(super) fn parse_duplicated(&mut self) -> ParseResult<Expr> {
        self.advance();
        self.expect(&Token::LParen)?;
        let mut exprs = vec![Box::new(self.parse_expr()?)];
        while self.eat(&Token::Comma) {
            exprs.push(Box::new(self.parse_expr()?));
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::Duplicated { exprs })
    }
}
