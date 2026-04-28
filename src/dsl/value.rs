use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt;

/// Parse a raw string into a [`Value`]: decimal if parseable, Null if empty, Text otherwise.
pub fn parse_value(s: String) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    match s.parse::<Decimal>() {
        Ok(d) => Value::Decimal(d),
        Err(_) => Value::Text(s),
    }
}

// ─────────────────────────────────────────────
// EVAL ERROR
// ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum EvalError {
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: &'static str, actual: String },

    #[error("division by zero")]
    DivisionByZero,

    #[error("unknown column: {0}")]
    UnknownColumn(String),

    #[error("unknown table: {0}")]
    UnknownTable(String),

    #[error("aggregate error: {0}")]
    AggregateError(String),

    #[error("sample error: {0}")]
    SampleError(String),
}

pub type EvalResult<T> = Result<T, EvalError>;

// ─────────────────────────────────────────────
// VALUE
// ─────────────────────────────────────────────

/// Runtime value produced by evaluating a DSL expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Decimal(Decimal),
    Bool(bool),
    Text(String),
    Null,
    List(Vec<Value>),
}

/// One data row: column_name → Value
pub type Row = HashMap<String, Value>;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Decimal(d) => write!(f, "{d}"),
            Value::Bool(b)    => write!(f, "{b}"),
            Value::Text(s)    => write!(f, "{s}"),
            Value::Null       => write!(f, "NULL"),
            Value::List(v)    => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
        }
    }
}

impl Value {
    /// Coerce to `Decimal`; returns `TypeMismatch` for non-numeric values.
    /// `Null` is treated as zero in arithmetic (SQL-style aggregate ignores Null separately).
    pub fn as_decimal(&self) -> EvalResult<Decimal> {
        match self {
            Value::Decimal(d) => Ok(*d),
            Value::Bool(b)    => Ok(if *b { Decimal::ONE } else { Decimal::ZERO }),
            Value::Null       => Ok(Decimal::ZERO),
            other => Err(EvalError::TypeMismatch {
                expected: "Decimal",
                actual: other.type_name(),
            }),
        }
    }

    /// Coerce to `bool`; returns `TypeMismatch` for non-boolean values.
    pub fn as_bool(&self) -> EvalResult<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Null    => Ok(false),
            other => Err(EvalError::TypeMismatch {
                expected: "Bool",
                actual: other.type_name(),
            }),
        }
    }

    /// Human-readable type name for error messages.
    pub fn type_name(&self) -> String {
        match self {
            Value::Decimal(_) => "Decimal".to_string(),
            Value::Bool(_)    => "Bool".to_string(),
            Value::Text(_)    => "Text".to_string(),
            Value::Null       => "Null".to_string(),
            Value::List(_)    => "List".to_string(),
        }
    }

    /// Coerce to `String`; any value has a text representation.
    pub fn as_text(&self) -> String {
        match self {
            Value::Text(s)    => s.clone(),
            Value::Decimal(d) => d.to_string(),
            Value::Bool(b)    => b.to_string(),
            Value::Null       => String::new(),
            Value::List(v)    => v.iter().map(|x| x.as_text()).collect::<Vec<_>>().join(", "),
        }
    }

    /// SQL-style equality: Null = Null → false, Null = anything → false.
    pub fn sql_eq(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Null, _) | (_, Value::Null) => false,
            (Value::Decimal(x), Value::Decimal(y)) => x == y,
            (Value::Bool(x), Value::Bool(y))       => x == y,
            (Value::Text(x), Value::Text(y))       => x == y,
            _ => false,
        }
    }

    /// Ordering comparison; returns `None` when either side is Null or types are incompatible.
    pub fn partial_cmp_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Value::Null, _) | (_, Value::Null) => None,
            (Value::Decimal(x), Value::Decimal(y)) => x.partial_cmp(y),
            (Value::Bool(x), Value::Bool(y))       => x.partial_cmp(y),
            (Value::Text(x), Value::Text(y))       => x.partial_cmp(y),
            _ => None,
        }
    }
}
