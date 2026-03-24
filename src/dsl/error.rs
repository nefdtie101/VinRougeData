use std::fmt;

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error at position {}: {}", self.position, self.message)
    }
}

impl ParseError {
    pub(crate) fn new(pos: usize, msg: impl Into<String>) -> Self {
        Self { message: msg.into(), position: pos }
    }
}

pub type ParseResult<T> = Result<T, ParseError>;
