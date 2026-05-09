use crate::dsl::ast::{CmpOp, Expr};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::result::AssertResult;
use super::Evaluator;

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

pub(super) fn cmp_op_display(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "=",
        CmpOp::NotEq => "<>",
        CmpOp::Gt => ">",
        CmpOp::Gte => ">=",
        CmpOp::Lt => "<",
        CmpOp::Lte => "<=",
    }
}

impl<'ds> Evaluator<'ds> {
    pub fn eval_assert(
        &self,
        label: &Option<String>,
        lhs: &Expr,
        rhs: &Expr,
        op: &CmpOp,
    ) -> EvalResult<AssertResult> {
        let empty = Row::new();

        // Heuristic: if the user wrote `ASSERT expr1 > expr2` without an explicit
        // `= true`, the parser produces `Assert { lhs: Compare{...}, rhs: true, op: Eq }`.
        // Show the inner comparison values instead of the boolean result.
        if *op == CmpOp::Eq {
            if let Ok(Value::Bool(true)) = self.eval(rhs, &empty) {
                if let Expr::Compare {
                    op: inner_op,
                    lhs: inner_lhs,
                    rhs: inner_rhs,
                } = lhs
                {
                    // Only take the aggregate-display shortcut when both sides
                    // resolve without a row context (i.e. they are aggregates or
                    // literals). If either side needs row data, fall through to
                    // the row-level evaluator below.
                    match (self.eval(inner_lhs, &empty), self.eval(inner_rhs, &empty)) {
                        (Ok(ilv), Ok(irv)) => {
                            let passed = self.apply_cmp(inner_op, &ilv, &irv);
                            return Ok(AssertResult {
                                label: label.clone(),
                                passed,
                                lhs_value: ilv.to_string(),
                                rhs_value: irv.to_string(),
                                op: cmp_op_display(inner_op).to_string(),
                                source_col: source_col_from_expr(inner_lhs)
                                    .or_else(|| source_col_from_expr(lhs)),
                            });
                        }
                        _ => {} // fall through to row-level eval
                    }
                }
            }
        }

        // Try aggregate-style evaluation (works for SUM/COUNT/AVG etc.)
        match self.eval(lhs, &empty) {
            Ok(lv) => {
                let rv = self.eval(rhs, &empty)?;
                let passed = self.apply_cmp(op, &lv, &rv);
                Ok(AssertResult {
                    label: label.clone(),
                    passed,
                    lhs_value: lv.to_string(),
                    rhs_value: rv.to_string(),
                    op: cmp_op_display(op).to_string(),
                    source_col: source_col_from_expr(lhs),
                })
            }
            // Row-level expression — column not found in empty row.
            // Only fall back to row-level evaluation for non-aggregate expressions;
            // aggregate failures (e.g. wrong column name in COUNT/SUM) should surface
            // their real error directly rather than producing a confusing "cannot infer table" message.
            Err(EvalError::UnknownColumn(_)) if !contains_aggregate(lhs) => {
                self.eval_row_assert(label, lhs, rhs, op)
            }
            Err(e) => Err(e),
        }
    }

    /// Evaluate a row-level ASSERT against all rows of the inferred table.
    /// Passes only when every row satisfies the condition.
    fn eval_row_assert(
        &self,
        label: &Option<String>,
        lhs: &Expr,
        rhs: &Expr,
        op: &CmpOp,
    ) -> EvalResult<AssertResult> {
        let table = table_from_expr(lhs).ok_or_else(|| {
            EvalError::AggregateError(
                "cannot infer table for row-level ASSERT — use table.column notation".to_string(),
            )
        })?;

        let rows = self.datasource.rows(table)?;

        // Evaluate rhs once (it should be a literal / aggregate, not row-dependent)
        let rv = self.eval(rhs, &Row::new()).unwrap_or(Value::Bool(true));

        let is_bool_assert = rv == Value::Bool(true) && *op == CmpOp::Eq;

        let total = rows.len();

        // Pre-check the first row so UnknownColumn errors surface immediately
        // rather than being swallowed inside a parallel loop.
        if let Some(first) = rows.first() {
            if let Err(EvalError::UnknownColumn(col)) = self.eval(lhs, first) {
                return Err(EvalError::UnknownColumn(col));
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let pass_count = rows
            .par_iter()
            .filter(|row| {
                if is_bool_assert {
                    self.eval(lhs, row)
                        .map(|v| v.as_bool().unwrap_or(false))
                        .unwrap_or(false)
                } else {
                    self.eval(lhs, row)
                        .map(|lv| self.apply_cmp(op, &lv, &rv))
                        .unwrap_or(false)
                }
            })
            .count();

        #[cfg(target_arch = "wasm32")]
        let pass_count = {
            let mut count = 0usize;
            for row in rows {
                let ok = if is_bool_assert {
                    match self.eval(lhs, row) {
                        Ok(v) => v.as_bool().unwrap_or(false),
                        Err(EvalError::UnknownColumn(col)) => {
                            return Err(EvalError::UnknownColumn(col))
                        }
                        Err(_) => false,
                    }
                } else {
                    match self.eval(lhs, row) {
                        Ok(lv) => self.apply_cmp(op, &lv, &rv),
                        Err(EvalError::UnknownColumn(col)) => {
                            return Err(EvalError::UnknownColumn(col))
                        }
                        Err(_) => false,
                    }
                };
                if ok {
                    count += 1;
                }
            }
            count
        };

        let passed = pass_count == total;
        let lhs_value = format!("{pass_count}/{total} rows pass");
        let rhs_value = if is_bool_assert {
            "all".to_string()
        } else {
            rv.to_string()
        };

        Ok(AssertResult {
            label: label.clone(),
            passed,
            lhs_value,
            rhs_value,
            op: if is_bool_assert {
                "=".to_string()
            } else {
                cmp_op_display(op).to_string()
            },
            source_col: source_col_from_expr(lhs),
        })
    }
}

/// Returns true if the expression tree contains an aggregate function call.
fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate { .. } => true,
        Expr::IsNull { expr, .. } => contains_aggregate(expr),
        Expr::Like { expr, .. } => contains_aggregate(expr),
        Expr::StringFn { expr, .. } => contains_aggregate(expr),
        Expr::SubStr {
            expr,
            start,
            length,
        } => {
            contains_aggregate(expr)
                || contains_aggregate(start)
                || length.as_deref().map(contains_aggregate).unwrap_or(false)
        }
        Expr::Concat { exprs } => exprs.iter().any(|e| contains_aggregate(e)),
        Expr::DateFn { expr } => contains_aggregate(expr),
        Expr::MathFn { expr, .. } => contains_aggregate(expr),
        Expr::NullIf { expr, .. } => contains_aggregate(expr),
        Expr::SaIdValid { expr } => contains_aggregate(expr),
        Expr::Not(inner) => contains_aggregate(inner),
        Expr::Compare { lhs, rhs, .. } => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::Logical { lhs, rhs, .. } => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::BinOp { lhs, rhs, .. } => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::InList { expr, .. } => contains_aggregate(expr),
        Expr::Between { expr, .. } => contains_aggregate(expr),
        Expr::Coalesce { exprs } => exprs.iter().any(|e| contains_aggregate(e)),
        Expr::Case {
            branches,
            else_expr,
        } => {
            branches
                .iter()
                .any(|(c, r)| contains_aggregate(c) || contains_aggregate(r))
                || else_expr
                    .as_deref()
                    .map(contains_aggregate)
                    .unwrap_or(false)
        }
        _ => false,
    }
}

/// Walk an expression tree and return the first `table.column` dotted ref found.
fn source_col_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::ColumnRef(name) if name.contains('.') => Some(name.clone()),
        Expr::IsNull { expr, .. } => source_col_from_expr(expr),
        Expr::IsBlank { expr, .. } => source_col_from_expr(expr),
        Expr::IsNumeric { expr, .. } => source_col_from_expr(expr),
        Expr::IsDate { expr, .. } => source_col_from_expr(expr),
        Expr::Like { expr, .. } => source_col_from_expr(expr),
        Expr::StringFn { expr, .. } => source_col_from_expr(expr),
        Expr::SubStr {
            expr,
            start,
            length,
        } => source_col_from_expr(expr)
            .or_else(|| source_col_from_expr(start))
            .or_else(|| length.as_deref().and_then(source_col_from_expr)),
        Expr::Concat { exprs } => exprs.iter().find_map(|e| source_col_from_expr(e)),
        Expr::DateFn { expr } => source_col_from_expr(expr),
        Expr::MathFn { expr, .. } => source_col_from_expr(expr),
        Expr::NullIf { expr, .. } => source_col_from_expr(expr),
        Expr::SaIdValid { expr } => source_col_from_expr(expr),
        Expr::Not(inner) => source_col_from_expr(inner),
        Expr::Duplicated { exprs } => exprs.iter().find_map(|e| source_col_from_expr(e)),
        Expr::InTableCol { expr, .. } => source_col_from_expr(expr),
        Expr::Compare { lhs, rhs, .. } => {
            source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs))
        }
        Expr::Logical { lhs, rhs, .. } => {
            source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs))
        }
        Expr::BinOp { lhs, rhs, .. } => {
            source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs))
        }
        Expr::InList { expr, .. } => source_col_from_expr(expr),
        Expr::Between { expr, .. } => source_col_from_expr(expr),
        Expr::Coalesce { exprs } => exprs.iter().find_map(|e| source_col_from_expr(e)),
        Expr::Case {
            branches,
            else_expr,
        } => branches
            .iter()
            .find_map(|(c, r)| source_col_from_expr(c).or_else(|| source_col_from_expr(r)))
            .or_else(|| else_expr.as_deref().and_then(source_col_from_expr)),
        Expr::Aggregate { expr, .. } => source_col_from_expr(expr),
        _ => None,
    }
}

/// Walk an expression tree and return the first table prefix found in a ColumnRef.
fn table_from_expr(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::ColumnRef(name) => name.find('.').map(|dot| &name[..dot]),
        Expr::IsNull { expr, .. } => table_from_expr(expr),
        Expr::IsBlank { expr, .. } => table_from_expr(expr),
        Expr::IsNumeric { expr, .. } => table_from_expr(expr),
        Expr::IsDate { expr, .. } => table_from_expr(expr),
        Expr::Like { expr, .. } => table_from_expr(expr),
        Expr::StringFn { expr, .. } => table_from_expr(expr),
        Expr::SubStr {
            expr,
            start,
            length,
        } => table_from_expr(expr)
            .or_else(|| table_from_expr(start))
            .or_else(|| length.as_deref().and_then(table_from_expr)),
        Expr::Concat { exprs } => exprs.iter().find_map(|e| table_from_expr(e)),
        Expr::DateFn { expr } => table_from_expr(expr),
        Expr::MathFn { expr, .. } => table_from_expr(expr),
        Expr::NullIf { expr, .. } => table_from_expr(expr),
        Expr::SaIdValid { expr } => table_from_expr(expr),
        Expr::Not(inner) => table_from_expr(inner),
        Expr::Duplicated { exprs } => exprs.iter().find_map(|e| table_from_expr(e)),
        Expr::InTableCol { expr, .. } => table_from_expr(expr),
        Expr::Aggregate { expr, .. } => table_from_expr(expr),
        Expr::Compare { lhs, rhs, .. } => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::Logical { lhs, rhs, .. } => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::BinOp { lhs, rhs, .. } => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::InList { expr, .. } => table_from_expr(expr),
        Expr::Between { expr, .. } => table_from_expr(expr),
        Expr::Coalesce { exprs } => exprs.iter().find_map(|e| table_from_expr(e)),
        Expr::Case {
            branches,
            else_expr,
        } => branches
            .iter()
            .find_map(|(c, r)| table_from_expr(c).or_else(|| table_from_expr(r)))
            .or_else(|| else_expr.as_deref().and_then(table_from_expr)),
        _ => None,
    }
}
