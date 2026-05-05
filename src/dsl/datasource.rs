use std::collections::HashMap;

use super::ast::{AggFunc, CmpOp, Expr, LogicOp};
use super::eval::{SchemaColumn, SchemaTable};
use super::value::{EvalError, EvalResult, Row, Value};

// ─────────────────────────────────────────────
// TRAIT
// ─────────────────────────────────────────────

/// Provides tabular row data to the evaluator.
///
/// Implementors are expected to load data before evaluation begins
/// (e.g. from CSV, Excel, or an in-memory Vec).  The trait is
/// intentionally synchronous so that `eval` can be called recursively
/// without async overhead, and so it remains WASM-compatible.
pub trait EvalDataSource: Send + Sync {
    /// Return all rows for the named table.
    ///
    /// Returns [`EvalError::UnknownTable`] if the table does not exist.
    fn rows(&self, table: &str) -> Result<&[Row], EvalError>;

    /// Return schema information for all loaded tables.
    fn schema_tables(&self) -> Vec<SchemaTable> {
        vec![]
    }

    /// Optionally push an aggregate computation down to the datasource.
    ///
    /// Return `Some(result)` if the datasource handled it (e.g. via a SQL query),
    /// or `None` to fall back to row-level iteration in the evaluator.
    /// The default implementation always returns `None`.
    fn try_aggregate(
        &self,
        _func: &AggFunc,
        _distinct: bool,
        _table: &str,
        _col: &str,
        _filter: &Option<Box<Expr>>,
    ) -> Option<EvalResult<Value>> {
        None
    }
}

// ─────────────────────────────────────────────
// SQL COMPILER
// ─────────────────────────────────────────────

/// Compile a DSL filter [`Expr`] to a SQL WHERE-clause fragment.
///
/// Returns `None` for expressions that cannot be represented in SQL
/// (e.g. custom predicates like `DUPLICATED`).  Column references are
/// stripped of their table prefix so they match DuckDB column names.
pub fn expr_to_sql(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Bool(b) => Some(b.to_string()),

        Expr::Compare { op, lhs, rhs } => {
            let l = atom_to_sql(lhs)?;
            let r = atom_to_sql(rhs)?;
            let op_str = match op {
                CmpOp::Eq    => "=",
                CmpOp::NotEq => "<>",
                CmpOp::Gt    => ">",
                CmpOp::Gte   => ">=",
                CmpOp::Lt    => "<",
                CmpOp::Lte   => "<=",
            };
            Some(format!("{l} {op_str} {r}"))
        }

        Expr::Logical { op, lhs, rhs } => {
            let l = expr_to_sql(lhs)?;
            let r = expr_to_sql(rhs)?;
            let op_str = match op {
                LogicOp::And => "AND",
                LogicOp::Or  => "OR",
            };
            Some(format!("({l}) {op_str} ({r})"))
        }

        Expr::Not(inner) => {
            let s = expr_to_sql(inner)?;
            Some(format!("NOT ({s})"))
        }

        Expr::IsNull { expr, negated } => {
            let col = atom_to_sql(expr)?;
            if *negated {
                Some(format!("{col} IS NOT NULL"))
            } else {
                Some(format!("{col} IS NULL"))
            }
        }

        Expr::InList { expr, values, negated } => {
            let col  = atom_to_sql(expr)?;
            let list = values.iter()
                .map(atom_to_sql)
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            if *negated {
                Some(format!("{col} NOT IN ({list})"))
            } else {
                Some(format!("{col} IN ({list})"))
            }
        }

        Expr::Between { expr, low, high, negated } => {
            let col = atom_to_sql(expr)?;
            let lo  = atom_to_sql(low)?;
            let hi  = atom_to_sql(high)?;
            if *negated {
                Some(format!("{col} NOT BETWEEN {lo} AND {hi}"))
            } else {
                Some(format!("{col} BETWEEN {lo} AND {hi}"))
            }
        }

        Expr::Like { expr, pattern, negated } => {
            let col = atom_to_sql(expr)?;
            let pat = atom_to_sql(pattern)?;
            if *negated {
                Some(format!("{col} NOT LIKE {pat}"))
            } else {
                Some(format!("{col} LIKE {pat}"))
            }
        }

        _ => None,
    }
}

/// Compile a leaf expression (column ref or literal) to a SQL fragment.
fn atom_to_sql(expr: &Expr) -> Option<String> {
    match expr {
        Expr::ColumnRef(name) => {
            let col = name.find('.').map(|d| &name[d + 1..]).unwrap_or(name.as_str());
            Some(format!("\"{}\"", col.replace('"', "\"\"")))
        }
        Expr::Number(d) => Some(d.to_string()),
        Expr::Str(s)    => Some(format!("'{}'", s.replace('\'', "''"))),
        Expr::Bool(b)   => Some(b.to_string()),
        Expr::Null      => Some("NULL".to_string()),
        _ => None,
    }
}

// ─────────────────────────────────────────────
// IN-MEMORY IMPLEMENTATION
// ─────────────────────────────────────────────

/// A simple in-memory data source backed by a `HashMap<table_name, Vec<Row>>`.
///
/// Intended for tests and for small scripting scenarios where data has
/// already been parsed into memory.
#[derive(Debug, Default)]
pub struct InMemoryDataSource {
    tables: HashMap<String, Vec<Row>>,
}

impl InMemoryDataSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a table with the given name and rows.
    /// Replaces any existing table with the same name.
    pub fn insert_table(&mut self, name: impl Into<String>, rows: Vec<Row>) {
        self.tables.insert(name.into(), rows);
    }
}

impl EvalDataSource for InMemoryDataSource {
    fn rows(&self, table: &str) -> Result<&[Row], EvalError> {
        self.tables
            .get(table)
            .map(|v| v.as_slice())
            .ok_or_else(|| EvalError::UnknownTable(table.to_string()))
    }

    fn schema_tables(&self) -> Vec<SchemaTable> {
        let mut tables: Vec<SchemaTable> = self.tables.iter().map(|(name, rows)| {
            let mut col_names: Vec<String> = rows
                .first()
                .map(|r| r.keys().cloned().collect())
                .unwrap_or_default();
            col_names.sort();
            let columns = col_names.into_iter().map(|c| SchemaColumn {
                col_type: rows.first()
                    .and_then(|r| r.get(&c))
                    .map(|v| match v {
                        Value::Decimal(_) => "numeric",
                        Value::Bool(_)    => "boolean",
                        Value::Null       => "null",
                        _                 => "text",
                    })
                    .unwrap_or("text")
                    .to_string(),
                name: c,
            }).collect();
            SchemaTable { name: name.clone(), columns, row_count: rows.len() }
        }).collect();
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        tables
    }
}
