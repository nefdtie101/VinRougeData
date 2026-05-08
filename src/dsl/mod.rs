// dsl — Vin Rouge Audit DSL

mod ast;
mod datasource;
mod error;
pub mod eval;
mod lexer;
mod parser;
mod resolver;
mod south_africa;
mod token;
mod value;

pub use ast::{AggFunc, ArithOp, CmpOp, Expr, LogicOp, SampleMethod, SampleSize, Statement};
pub use datasource::{expr_to_sql, EvalDataSource, InMemoryDataSource};
pub use error::{ParseError, ParseResult};
pub use eval::{run_script, AssertResult, ChartResult, Evaluator, SampleResult, SchemaColumn, SchemaTable, StatementResult};
pub use resolver::{resolve, ResolveError, Resolver, Schema};
pub use lexer::Lexer;
pub use parser::Parser;
pub use value::{parse_value, EvalError, EvalResult, Row, Value};

/// Parse a DSL string into a list of [`Statement`]s.
///
/// # Example
/// ```rust
/// let stmts = vinrouge::dsl::parse(
///     r#"
///     debtors_check: ASSERT "Debtors reconciliation"
///         SUM(invoices.amount) WHERE status = "open"
///         = debtors_control
///
///     sample_large: SAMPLE MUS invoices.amount 50
///         WHERE amount > 10000
///     "#
/// ).unwrap();
/// ```
pub fn parse(input: &str) -> ParseResult<Vec<Statement>> {
    let tokens = Lexer::new(input).tokenise()?;
    Parser::new(tokens).parse_script()
}

#[cfg(test)]
mod tests;
