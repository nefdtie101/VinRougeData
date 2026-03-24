use rust_decimal::Decimal;
use std::str::FromStr;

use super::error::{ParseError, ParseResult};
use super::token::Token;

pub struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            pos: 0,
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, c)| *c)
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next();
        if let Some((i, _)) = next {
            self.pos = i;
        }
        next
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_char(), Some(' ' | '\t' | '\n' | '\r')) {
            self.next_char();
        }
    }

    fn skip_line_comment(&mut self) {
        while !matches!(self.peek_char(), Some('\n') | None) {
            self.next_char();
        }
    }

    fn read_ident_or_keyword(&mut self, start: usize) -> Token {
        while matches!(self.peek_char(), Some(c) if c.is_alphanumeric() || c == '_' || c == '.') {
            self.next_char();
        }
        let end = self.chars.peek().map(|(i, _)| *i).unwrap_or(self.input.len());
        let word = &self.input[start..end];

        match word.to_uppercase().as_str() {
            "SUM"        => Token::Sum,
            "AVG"        => Token::Avg,
            "COUNT"      => Token::Count,
            "MIN"        => Token::Min,
            "MAX"        => Token::Max,
            "ASSERT"     => Token::Assert,
            "SAMPLE"     => Token::Sample,
            "WHERE"      => Token::Where,
            "AND"        => Token::And,
            "OR"         => Token::Or,
            "NOT"        => Token::Not,
            "IN"         => Token::In,
            "BETWEEN"    => Token::Between,
            "IS"         => Token::Is,
            "NULL"       => Token::Null,
            "TRUE"       => Token::True,
            "FALSE"      => Token::False,
            "MUS"        => Token::Mus,
            "RANDOM"     => Token::Random,
            "SYSTEMATIC" => Token::Systematic,
            "STRATIFIED" => Token::Stratified,
            "TOP"        => Token::Top,
            _            => Token::Ident(word.to_string()),
        }
    }

    fn read_number(&mut self, start: usize) -> ParseResult<Token> {
        while matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
            self.next_char();
        }
        if self.peek_char() == Some('.') {
            self.next_char();
            while matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
                self.next_char();
            }
        }
        let end = self.chars.peek().map(|(i, _)| *i).unwrap_or(self.input.len());
        let s = &self.input[start..end];
        Decimal::from_str(s)
            .map(Token::Number)
            .map_err(|_| ParseError::new(start, format!("invalid number '{s}'")))
    }

    fn read_string(&mut self, start: usize) -> ParseResult<Token> {
        let mut s = String::new();
        loop {
            match self.next_char() {
                Some((_, '"')) => return Ok(Token::StringLit(s)),
                Some((_, c))   => s.push(c),
                None           => return Err(ParseError::new(start, "unterminated string literal")),
            }
        }
    }

    /// Tokenise the full input into a `Vec<(position, Token)>`.
    pub fn tokenise(&mut self) -> ParseResult<Vec<(usize, Token)>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            let pos = self.chars.peek().map(|(i, _)| *i).unwrap_or(self.input.len());

            match self.next_char() {
                None => { tokens.push((pos, Token::Eof)); break; }
                Some((_, '-')) if self.peek_char() == Some('-') => {
                    self.skip_line_comment();
                }
                Some((i, c)) if c.is_alphabetic() || c == '_' => {
                    tokens.push((i, self.read_ident_or_keyword(i)));
                }
                Some((i, c)) if c.is_ascii_digit() => {
                    tokens.push((i, self.read_number(i)?));
                }
                Some((i, '"')) => {
                    tokens.push((i, self.read_string(i)?));
                }
                Some((i, '+')) => tokens.push((i, Token::Plus)),
                Some((i, '-')) => tokens.push((i, Token::Minus)),
                Some((i, '*')) => tokens.push((i, Token::Star)),
                Some((i, '/')) => tokens.push((i, Token::Slash)),
                Some((i, '%')) => tokens.push((i, Token::Percent)),
                Some((i, '(')) => tokens.push((i, Token::LParen)),
                Some((i, ')')) => tokens.push((i, Token::RParen)),
                Some((i, ',')) => tokens.push((i, Token::Comma)),
                Some((i, ':')) => tokens.push((i, Token::Colon)),
                Some((i, '=')) => tokens.push((i, Token::Eq)),
                Some((i, '>')) => {
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        tokens.push((i, Token::Gte));
                    } else {
                        tokens.push((i, Token::Gt));
                    }
                }
                Some((i, '<')) => {
                    match self.peek_char() {
                        Some('=') => { self.next_char(); tokens.push((i, Token::Lte)); }
                        Some('>') => { self.next_char(); tokens.push((i, Token::NotEq)); }
                        _         => tokens.push((i, Token::Lt)),
                    }
                }
                Some((i, c)) => {
                    return Err(ParseError::new(i, format!("unexpected character '{c}'")));
                }
            }
        }
        Ok(tokens)
    }
}
