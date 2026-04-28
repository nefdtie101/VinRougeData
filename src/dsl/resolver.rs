use std::collections::{HashMap, HashSet};

use super::ast::*;

// ─────────────────────────────────────────────
// SCHEMA
// ─────────────────────────────────────────────

/// Describes the shape of available data: table name → set of column names.
///
/// Column names are stored lower-cased so lookups are always case-insensitive.
#[derive(Debug, Default, Clone)]
pub struct Schema {
    tables: HashMap<String, HashSet<String>>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a table with the given column names.
    /// Replaces any existing table with the same name.
    pub fn add_table(
        &mut self,
        table: impl Into<String>,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.tables.insert(
            table.into().to_lowercase(),
            columns.into_iter().map(|c| c.into().to_lowercase()).collect(),
        );
    }

    /// Check whether a table exists.
    pub fn has_table(&self, table: &str) -> bool {
        self.tables.contains_key(&table.to_lowercase())
    }

    /// Check whether a column exists in a table.
    pub fn has_column(&self, table: &str, column: &str) -> bool {
        self.tables
            .get(&table.to_lowercase())
            .map(|cols| cols.contains(&column.to_lowercase()))
            .unwrap_or(false)
    }
}

// ─────────────────────────────────────────────
// ERRORS
// ─────────────────────────────────────────────

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown table '{table}' referenced in '{reference}'")]
    UnknownTable { table: String, reference: String },

    #[error("unknown column '{column}' in table '{table}'")]
    UnknownColumn { table: String, column: String },

    #[error("bare column reference '{0}' — use table.column notation inside aggregates")]
    BareColumnInAggregate(String),

    #[error("column reference '{0}' has no table prefix — cannot resolve without a context table")]
    AmbiguousColumn(String),
}

// ─────────────────────────────────────────────
// RESOLVER
// ─────────────────────────────────────────────

/// Walks a parsed AST and validates every column reference against a [`Schema`].
///
/// Returns a list of all errors found — the entire script is checked even if
/// earlier references are invalid, so the caller gets a full error list at once.
pub struct Resolver<'s> {
    schema: &'s Schema,
    errors: Vec<ResolveError>,
}

impl<'s> Resolver<'s> {
    pub fn new(schema: &'s Schema) -> Self {
        Self { schema, errors: Vec::new() }
    }

    /// Validate a list of statements and return all resolve errors found.
    pub fn resolve(mut self, statements: &[Statement]) -> Vec<ResolveError> {
        for stmt in statements {
            self.check_expr(&stmt.expr);
        }
        self.errors
    }

    // ── Expression walker ─────────────────────

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            // Literals — nothing to resolve
            Expr::Number(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Null => {}

            // Bare column ref outside an aggregate — allowed as long as the table
            // prefix is present so we can look it up.
            Expr::ColumnRef(name) => self.check_column_ref(name, false),

            Expr::BinOp { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }

            Expr::Compare { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }

            Expr::Logical { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }

            Expr::Not(inner) => self.check_expr(inner),

            Expr::InList { expr, values, .. } => {
                self.check_expr(expr);
                for v in values {
                    self.check_expr(v);
                }
            }

            Expr::Between { expr, low, high, .. } => {
                self.check_expr(expr);
                self.check_expr(low);
                self.check_expr(high);
            }

            Expr::IsNull { expr, .. } => self.check_expr(expr),

            // Aggregate — inner expr must be a table.column ref
            Expr::Aggregate { expr, filter, .. } => {
                self.check_aggregate_expr(expr);
                if let Some(f) = filter {
                    self.check_expr(f);
                }
            }

            Expr::Coalesce { exprs } => {
                for e in exprs { self.check_expr(e); }
            }

            Expr::NullIf { expr, compare } => {
                self.check_expr(expr);
                self.check_expr(compare);
            }

            Expr::MathFn { expr, scale, .. } => {
                self.check_expr(expr);
                if let Some(s) = scale { self.check_expr(s); }
            }

            Expr::Like { expr, pattern, .. } => {
                self.check_expr(expr);
                self.check_expr(pattern);
            }

            Expr::StringFn { expr, .. } => self.check_expr(expr),

            Expr::DateFn { expr } => self.check_expr(expr),

            Expr::Case { branches, else_expr } => {
                for (cond, result) in branches {
                    self.check_expr(cond);
                    self.check_expr(result);
                }
                if let Some(e) = else_expr {
                    self.check_expr(e);
                }
            }

            // Assert — recurse both sides
            Expr::Assert { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }

            // Sample — check value_column and filter
            Expr::Sample { population, value_column, filter, .. } => {
                self.check_sample_column(population, value_column);
                if let Some(f) = filter {
                    self.check_expr(f);
                }
            }
        }
    }

    // ── Column reference checks ───────────────

    /// Validate a `table.column` reference (or bare column if `require_prefix` is false).
    fn check_column_ref(&mut self, name: &str, require_prefix: bool) {
        match name.find('.') {
            Some(dot) => {
                let table = &name[..dot];
                let column = &name[dot + 1..];
                if !self.schema.has_table(table) {
                    self.errors.push(ResolveError::UnknownTable {
                        table: table.to_string(),
                        reference: name.to_string(),
                    });
                } else if !self.schema.has_column(table, column) {
                    self.errors.push(ResolveError::UnknownColumn {
                        table: table.to_string(),
                        column: column.to_string(),
                    });
                }
            }
            None if require_prefix => {
                self.errors.push(ResolveError::BareColumnInAggregate(name.to_string()));
            }
            None => {
                // Bare column ref outside aggregate — ambiguous but not always wrong
                // (e.g. evaluated against a context row).  Emit a warning-level error
                // only if the schema has no table that owns this column.
                let found = self.schema.tables.values().any(|cols| {
                    cols.contains(&name.to_lowercase())
                });
                if !found && !self.schema.tables.is_empty() {
                    self.errors.push(ResolveError::AmbiguousColumn(name.to_string()));
                }
            }
        }
    }

    /// Aggregate inner expressions must be `table.column` refs.
    fn check_aggregate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::ColumnRef(name) => self.check_column_ref(name, true),
            // Arithmetic inside an aggregate (e.g. SUM(a.x + a.y)) — recurse
            Expr::BinOp { lhs, rhs, .. } => {
                self.check_aggregate_expr(lhs);
                self.check_aggregate_expr(rhs);
            }
            other => self.check_expr(other),
        }
    }

    /// Validate the value column used in a SAMPLE statement.
    fn check_sample_column(&mut self, population: &str, value_column: &str) {
        let col = if let Some(dot) = value_column.find('.') {
            let tbl = &value_column[..dot];
            // Table prefix must match population
            if !tbl.eq_ignore_ascii_case(population) && self.schema.has_table(population) {
                // Still check the prefixed reference
            }
            &value_column[dot + 1..]
        } else {
            value_column
        };

        if !self.schema.has_table(population) {
            self.errors.push(ResolveError::UnknownTable {
                table: population.to_string(),
                reference: value_column.to_string(),
            });
        } else if !self.schema.has_column(population, col) {
            self.errors.push(ResolveError::UnknownColumn {
                table: population.to_string(),
                column: col.to_string(),
            });
        }
    }
}

// ─────────────────────────────────────────────
// CONVENIENCE FUNCTION
// ─────────────────────────────────────────────

/// Resolve all column references in `statements` against `schema`.
///
/// Returns an empty `Vec` if everything is valid.
pub fn resolve(statements: &[Statement], schema: &Schema) -> Vec<ResolveError> {
    Resolver::new(schema).resolve(statements)
}
