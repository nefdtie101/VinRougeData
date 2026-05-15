use super::errors::ResolveError;
use super::schema::Schema;
use crate::dsl::ast::*;

const VALID_CHART_TYPES: &[&str] = &["bar", "line", "pie", "scatter"];

/// Walks a parsed AST and validates every column reference against a [`Schema`].
///
/// Returns a list of all errors found — the entire script is checked even if
/// earlier references are invalid, so the caller gets a full error list at once.
pub struct Resolver<'s> {
    pub(super) schema: &'s Schema,
    pub(super) errors: Vec<ResolveError>,
}

impl<'s> Resolver<'s> {
    pub fn new(schema: &'s Schema) -> Self {
        Self {
            schema,
            errors: Vec::new(),
        }
    }

    /// Validate a list of statements and return all resolve errors found.
    pub fn resolve(mut self, statements: &[Statement]) -> Vec<ResolveError> {
        for stmt in statements {
            self.check_expr(&stmt.expr);
        }
        self.errors
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Number(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Null => {}

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

            Expr::Between {
                expr, low, high, ..
            } => {
                self.check_expr(expr);
                self.check_expr(low);
                self.check_expr(high);
            }

            Expr::IsNull { expr, .. } => self.check_expr(expr),

            Expr::IsBlank { expr, .. } => self.check_expr(expr),
            Expr::IsNumeric { expr, .. } => self.check_expr(expr),
            Expr::IsDate { expr, .. } => self.check_expr(expr),
            Expr::SaIdValid { expr } => self.check_expr(expr),

            Expr::Duplicated { exprs } => {
                for e in exprs {
                    self.check_expr(e);
                }
            }

            Expr::InTableCol {
                expr,
                table,
                column,
                ..
            } => {
                self.check_expr(expr);
                if !self.schema.has_table(table) {
                    self.errors.push(ResolveError::UnknownTable {
                        table: table.clone(),
                        reference: format!("{table}.{column}"),
                    });
                } else if !self.schema.has_column(table, column) {
                    self.errors.push(ResolveError::UnknownColumn {
                        table: table.clone(),
                        column: column.clone(),
                    });
                }
            }

            Expr::RelationDecl {
                from_table,
                from_col,
                to_table,
                to_col,
            } => {
                if !self.schema.has_table(from_table) {
                    self.errors.push(ResolveError::UnknownTable {
                        table: from_table.clone(),
                        reference: format!("{from_table}.{from_col}"),
                    });
                } else if !self.schema.has_column(from_table, from_col) {
                    self.errors.push(ResolveError::UnknownColumn {
                        table: from_table.clone(),
                        column: from_col.clone(),
                    });
                }
                if !self.schema.has_table(to_table) {
                    self.errors.push(ResolveError::UnknownTable {
                        table: to_table.clone(),
                        reference: format!("{to_table}.{to_col}"),
                    });
                } else if !self.schema.has_column(to_table, to_col) {
                    self.errors.push(ResolveError::UnknownColumn {
                        table: to_table.clone(),
                        column: to_col.clone(),
                    });
                }
            }

            Expr::Aggregate { expr, filter, .. } => {
                self.check_aggregate_expr(expr);
                if let Some(f) = filter {
                    self.check_expr(f);
                }
            }

            Expr::Coalesce { exprs } => {
                for e in exprs {
                    self.check_expr(e);
                }
            }

            Expr::NullIf { expr, compare } => {
                self.check_expr(expr);
                self.check_expr(compare);
            }

            Expr::MathFn { expr, scale, .. } => {
                self.check_expr(expr);
                if let Some(s) = scale {
                    self.check_expr(s);
                }
            }

            Expr::Like { expr, pattern, .. } => {
                self.check_expr(expr);
                self.check_expr(pattern);
            }

            Expr::StringFn { expr, .. } => self.check_expr(expr),

            Expr::SubStr {
                expr,
                start,
                length,
            } => {
                self.check_expr(expr);
                self.check_expr(start);
                if let Some(l) = length {
                    self.check_expr(l);
                }
            }

            Expr::Concat { exprs } => {
                for e in exprs {
                    self.check_expr(e);
                }
            }

            Expr::DateFn { expr } => self.check_expr(expr),

            Expr::Case {
                branches,
                else_expr,
            } => {
                for (cond, result) in branches {
                    self.check_expr(cond);
                    self.check_expr(result);
                }
                if let Some(e) = else_expr {
                    self.check_expr(e);
                }
            }

            Expr::Assert { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }

            Expr::Sample {
                population,
                value_column,
                filter,
                ..
            } => {
                self.check_sample_column(population, value_column);
                if let Some(f) = filter {
                    self.check_expr(f);
                }
            }

            Expr::Chart {
                chart_type,
                aggregate,
                dimension,
            } => {
                if !VALID_CHART_TYPES.contains(&chart_type.to_lowercase().as_str()) {
                    self.errors
                        .push(ResolveError::InvalidChartType(chart_type.clone()));
                }
                if !matches!(aggregate.as_ref(), Expr::Aggregate { .. }) {
                    self.errors
                        .push(ResolveError::InvalidChartAggregate(format!(
                            "{aggregate:?}"
                        )));
                }
                self.check_expr(aggregate);
                self.check_expr(dimension);
            }

            Expr::Section { statements, .. } => {
                for stmt in statements {
                    self.check_expr(&stmt.expr);
                }
            }

            Expr::Schema => {}

            Expr::ShowRows { table, filter } => {
                if !self.schema.has_table(table) && !self.schema.tables.is_empty() {
                    self.errors.push(ResolveError::UnknownTable {
                        table: table.clone(),
                        reference: table.clone(),
                    });
                }
                if let Some(f) = filter {
                    self.check_expr(f);
                }
            }
        }
    }

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
                self.errors
                    .push(ResolveError::BareColumnInAggregate(name.to_string()));
            }
            None => {
                let found = self
                    .schema
                    .tables
                    .values()
                    .any(|cols| cols.contains(&name.to_lowercase()));
                if !found && !self.schema.tables.is_empty() {
                    self.errors
                        .push(ResolveError::AmbiguousColumn(name.to_string()));
                }
            }
        }
    }

    fn check_aggregate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::ColumnRef(name) => self.check_column_ref(name, true),
            Expr::BinOp { lhs, rhs, .. } => {
                self.check_aggregate_expr(lhs);
                self.check_aggregate_expr(rhs);
            }
            other => self.check_expr(other),
        }
    }

    fn check_sample_column(&mut self, population: &str, value_column: &str) {
        let col = if let Some(dot) = value_column.find('.') {
            let tbl = &value_column[..dot];
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
