use crate::dsl::ast::{CmpOp, Expr};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::result::AssertResult;
use super::Evaluator;

pub(super) fn cmp_op_display(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq    => "=",
        CmpOp::NotEq => "<>",
        CmpOp::Gt    => ">",
        CmpOp::Gte   => ">=",
        CmpOp::Lt    => "<",
        CmpOp::Lte   => "<=",
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

        // Try aggregate-style evaluation (works for SUM/COUNT/AVG etc.)
        match self.eval(lhs, &empty) {
            Ok(lv) => {
                let rv = self.eval(rhs, &empty)?;
                let passed = self.apply_cmp(op, &lv, &rv);
                Ok(AssertResult {
                    label:      label.clone(),
                    passed,
                    lhs_value:  lv.to_string(),
                    rhs_value:  rv.to_string(),
                    op:         cmp_op_display(op).to_string(),
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

        let mut pass_count = 0usize;
        let total = rows.len();

        for row in rows {
            let ok = if is_bool_assert {
                match self.eval(lhs, row) {
                    Ok(v)  => v.as_bool().unwrap_or(false),
                    // Propagate missing-column immediately: it means the column does not
                    // exist in the table at all, not that the assertion value is false.
                    Err(EvalError::UnknownColumn(col)) => return Err(EvalError::UnknownColumn(col)),
                    Err(_) => false,
                }
            } else {
                match self.eval(lhs, row) {
                    Ok(lv) => self.apply_cmp(op, &lv, &rv),
                    Err(EvalError::UnknownColumn(col)) => return Err(EvalError::UnknownColumn(col)),
                    Err(_) => false,
                }
            };
            if ok { pass_count += 1; }
        }

        let passed = pass_count == total;
        let lhs_value = format!("{pass_count}/{total} rows pass");
        let rhs_value = if is_bool_assert { "all".to_string() } else { rv.to_string() };

        Ok(AssertResult {
            label: label.clone(),
            passed,
            lhs_value,
            rhs_value,
            op: if is_bool_assert { "=".to_string() } else { cmp_op_display(op).to_string() },
            source_col: source_col_from_expr(lhs),
        })
    }
}

/// Returns true if the expression tree contains an aggregate function call.
fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate { .. }              => true,
        Expr::IsNull   { expr, .. }         => contains_aggregate(expr),
        Expr::Like     { expr, .. }         => contains_aggregate(expr),
        Expr::StringFn { expr, .. }         => contains_aggregate(expr),
        Expr::DateFn   { expr }             => contains_aggregate(expr),
        Expr::MathFn   { expr, .. }         => contains_aggregate(expr),
        Expr::NullIf   { expr, .. }         => contains_aggregate(expr),
        Expr::Not      (inner)              => contains_aggregate(inner),
        Expr::Compare  { lhs, rhs, .. }     => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::Logical  { lhs, rhs, .. }     => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::BinOp    { lhs, rhs, .. }     => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::InList   { expr, .. }         => contains_aggregate(expr),
        Expr::Between  { expr, .. }         => contains_aggregate(expr),
        Expr::Coalesce { exprs }            => exprs.iter().any(|e| contains_aggregate(e)),
        Expr::Case { branches, else_expr }  => {
            branches.iter().any(|(c, r)| contains_aggregate(c) || contains_aggregate(r))
                || else_expr.as_deref().map(contains_aggregate).unwrap_or(false)
        }
        _ => false,
    }
}

/// Walk an expression tree and return the first `table.column` dotted ref found.
fn source_col_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::ColumnRef(name) if name.contains('.') => Some(name.clone()),
        Expr::IsNull   { expr, .. }     => source_col_from_expr(expr),
        Expr::Like     { expr, .. }     => source_col_from_expr(expr),
        Expr::StringFn { expr, .. }     => source_col_from_expr(expr),
        Expr::DateFn   { expr }         => source_col_from_expr(expr),
        Expr::MathFn   { expr, .. }     => source_col_from_expr(expr),
        Expr::NullIf   { expr, .. }     => source_col_from_expr(expr),
        Expr::Not      (inner)          => source_col_from_expr(inner),
        Expr::Compare  { lhs, rhs, .. } => source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs)),
        Expr::Logical  { lhs, rhs, .. } => source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs)),
        Expr::BinOp    { lhs, rhs, .. } => source_col_from_expr(lhs).or_else(|| source_col_from_expr(rhs)),
        Expr::InList   { expr, .. }     => source_col_from_expr(expr),
        Expr::Between  { expr, .. }     => source_col_from_expr(expr),
        Expr::Coalesce { exprs }        => exprs.iter().find_map(|e| source_col_from_expr(e)),
        Expr::Case { branches, else_expr } => {
            branches.iter().find_map(|(c, r)| source_col_from_expr(c).or_else(|| source_col_from_expr(r)))
                .or_else(|| else_expr.as_deref().and_then(source_col_from_expr))
        }
        Expr::Aggregate { expr, .. } => source_col_from_expr(expr),
        _ => None,
    }
}

/// Walk an expression tree and return the first table prefix found in a ColumnRef.
fn table_from_expr(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::ColumnRef(name) => {
            name.find('.').map(|dot| &name[..dot])
        }
        Expr::IsNull      { expr, .. }         => table_from_expr(expr),
        Expr::Like        { expr, .. }         => table_from_expr(expr),
        Expr::StringFn    { expr, .. }         => table_from_expr(expr),
        Expr::DateFn      { expr }             => table_from_expr(expr),
        Expr::MathFn      { expr, .. }         => table_from_expr(expr),
        Expr::NullIf      { expr, .. }         => table_from_expr(expr),
        Expr::Not         (inner)              => table_from_expr(inner),
        Expr::Aggregate   { expr, .. }         => table_from_expr(expr),
        Expr::Compare     { lhs, rhs, .. }     => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::Logical     { lhs, rhs, .. }     => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::BinOp       { lhs, rhs, .. }     => table_from_expr(lhs).or_else(|| table_from_expr(rhs)),
        Expr::InList      { expr, .. }         => table_from_expr(expr),
        Expr::Between     { expr, .. }         => table_from_expr(expr),
        Expr::Coalesce    { exprs }            => exprs.iter().find_map(|e| table_from_expr(e)),
        Expr::Case        { branches, else_expr } => {
            branches.iter().find_map(|(c, r)| table_from_expr(c).or_else(|| table_from_expr(r)))
                .or_else(|| else_expr.as_deref().and_then(table_from_expr))
        }
        _ => None,
    }
}
