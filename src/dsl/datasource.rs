use std::collections::HashMap;

use super::value::{EvalError, Row};

// ─────────────────────────────────────────────
// TRAIT
// ─────────────────────────────────────────────

/// Provides tabular row data to the evaluator.
///
/// Implementors are expected to load data before evaluation begins
/// (e.g. from CSV, Excel, or an in-memory Vec).  The trait is
/// intentionally synchronous so that `eval` can be called recursively
/// without async overhead, and so it remains WASM-compatible.
pub trait EvalDataSource {
    /// Return all rows for the named table.
    ///
    /// Returns [`EvalError::UnknownTable`] if the table does not exist.
    fn rows(&self, table: &str) -> Result<&[Row], EvalError>;
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
}
