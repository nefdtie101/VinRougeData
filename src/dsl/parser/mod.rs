use crate::dsl::ast::*;
use crate::dsl::error::{ParseError, ParseResult};
use crate::dsl::token::Token;

mod builtins;
mod exprs;
mod stmts;

pub struct Parser {
    pub(super) tokens: Vec<(usize, Token)>,
    pub(super) pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(usize, Token)>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub(super) fn peek(&self) -> &Token {
        &self.tokens[self.pos].1
    }

    pub(super) fn peek_next(&self) -> &Token {
        self.tokens
            .get(self.pos + 1)
            .map(|(_, t)| t)
            .unwrap_or(&Token::Eof)
    }

    pub(super) fn peek_pos(&self) -> usize {
        self.tokens[self.pos].0
    }

    pub(super) fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos].1;
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    pub(super) fn expect(&mut self, expected: &Token) -> ParseResult<()> {
        let pos = self.peek_pos();
        let got = self.peek().clone();
        if &got == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::new(
                pos,
                format!("expected {expected}, got {got}"),
            ))
        }
    }

    pub(super) fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume `SHOW FAILURES IN TABLE` and return true, or return false if absent.
    pub(super) fn eat_show_failures_in_table(&mut self) -> bool {
        if self.peek() == &Token::Show {
            let saved = self.pos;
            self.advance(); // SHOW
            if self.peek() == &Token::Failures {
                self.advance(); // FAILURES
                if self.peek() == &Token::In {
                    self.advance(); // IN
                    if self.peek() == &Token::Table {
                        self.advance(); // TABLE
                        return true;
                    }
                }
            }
            self.pos = saved; // not the sequence — back up
        }
        false
    }

    pub fn parse_script(&mut self) -> ParseResult<Vec<Statement>> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

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
        } else if let Token::StringLit(s) = self.peek().clone() {
            if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].1 == Token::Colon {
                self.advance(); // consume string
                self.advance(); // consume ':'
                Some(s)
            } else {
                None
            }
        } else {
            None
        };

        let expr = self.parse_expr()?;
        Ok(Statement { label, expr })
    }
}
