use rust_decimal::Decimal;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Keywords
    Sum,
    Avg,
    Count,
    Min,
    Max,
    Assert,
    Sample,
    Where,
    And,
    Or,
    Not,
    In,
    Between,
    Is,
    Null,
    True,
    False,
    Mus,        // Monetary Unit Sampling
    Random,
    Systematic,
    Stratified,
    Top,        // Top stratum in MUS
    From,       // FROM keyword in SAMPLE
    Size,       // SIZE keyword in SAMPLE
    Distinct,   // DISTINCT modifier in COUNT(DISTINCT ...)
    Like,       // LIKE pattern match
    Upper,      // UPPER() string function
    Lower,      // LOWER() string function
    Trim,       // TRIM() string function
    Length,     // LENGTH() string function
    Date,       // DATE() normalization function
    Case,       // CASE expression
    When,       // WHEN clause
    Then,       // THEN clause
    Else,       // ELSE clause
    End,        // END of CASE
    Coalesce,   // COALESCE(a, b, ...) — first non-null
    NullIf,     // NULLIF(a, b) — null if a = b
    Iif,        // IIF(cond, then, else) — inline if
    Abs,        // ABS(expr) — absolute value
    Round,      // ROUND(expr, scale) — decimal rounding
    CountIf,    // COUNTIF(col, criteria) — Excel-style conditional count
    SumIf,      // SUMIF(range, criteria, sum_col) — Excel-style conditional sum

    // Identifiers and literals
    Ident(String),      // table.column or plain name
    Number(Decimal),
    StringLit(String),  // "some string"

    // Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Comparison operators
    Eq,     // =
    NotEq,  // <>
    Gt,     // >
    Gte,    // >=
    Lt,     // <
    Lte,    // <=

    // Delimiters
    LParen,
    RParen,
    Comma,
    Dot,
    Colon,

    // End of input
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(s)     => write!(f, "identifier '{s}'"),
            Token::Number(n)    => write!(f, "number {n}"),
            Token::StringLit(s) => write!(f, "string \"{s}\""),
            Token::Plus         => write!(f, "'+'"),
            Token::Minus        => write!(f, "'-'"),
            Token::Star         => write!(f, "'*'"),
            Token::Slash        => write!(f, "'/'"),
            Token::Eq           => write!(f, "'='"),
            Token::NotEq        => write!(f, "'<>'"),
            Token::Gt           => write!(f, "'>'"),
            Token::Gte          => write!(f, "'>='"),
            Token::Lt           => write!(f, "'<'"),
            Token::Lte          => write!(f, "'<='"),
            Token::LParen       => write!(f, "'('"),
            Token::RParen       => write!(f, "')'"),
            Token::Comma        => write!(f, "','"),
            Token::Eof          => write!(f, "end of input"),
            other               => write!(f, "{other:?}"),
        }
    }
}
