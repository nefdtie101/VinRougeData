mod aggregate;
mod assert;
mod result;
mod sample;

pub use result::{AssertResult, SampleResult, StatementResult};

use crate::dsl::ast::*;
use crate::dsl::datasource::EvalDataSource;
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

// ─────────────────────────────────────────────
// EVALUATOR
// ─────────────────────────────────────────────

pub struct Evaluator<'ds> {
    pub(super) datasource: &'ds dyn EvalDataSource,
}

impl<'ds> Evaluator<'ds> {
    pub fn new(datasource: &'ds dyn EvalDataSource) -> Self {
        Self { datasource }
    }

    // ── Column resolution ─────────────────────

    pub(super) fn resolve_column<'r>(name: &str, row: &'r Row) -> EvalResult<&'r Value> {
        let col = if let Some(dot) = name.find('.') { &name[dot + 1..] } else { name };
        row.get(col)
            .or_else(|| {
                row.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(col))
                    .map(|(_, v)| v)
            })
            .ok_or_else(|| EvalError::UnknownColumn(name.to_string()))
    }

    // ── Comparison helper ─────────────────────

    pub(super) fn apply_cmp(&self, op: &CmpOp, l: &Value, r: &Value) -> bool {
        match op {
            CmpOp::Eq    => Value::sql_eq(l, r),
            CmpOp::NotEq => !Value::sql_eq(l, r),
            CmpOp::Gt    => Value::partial_cmp_values(l, r)
                                .map(|o| o == std::cmp::Ordering::Greater)
                                .unwrap_or(false),
            CmpOp::Gte   => Value::partial_cmp_values(l, r)
                                .map(|o| o != std::cmp::Ordering::Less)
                                .unwrap_or(false),
            CmpOp::Lt    => Value::partial_cmp_values(l, r)
                                .map(|o| o == std::cmp::Ordering::Less)
                                .unwrap_or(false),
            CmpOp::Lte   => Value::partial_cmp_values(l, r)
                                .map(|o| o != std::cmp::Ordering::Greater)
                                .unwrap_or(false),
        }
    }

    // ── Main dispatch ─────────────────────────

    /// Evaluate a scalar expression against a context row.
    pub fn eval(&self, expr: &Expr, row: &Row) -> EvalResult<Value> {
        match expr {
            Expr::Number(d)  => Ok(Value::Decimal(*d)),
            Expr::Bool(b)    => Ok(Value::Bool(*b)),
            Expr::Str(s)     => Ok(Value::Text(s.clone())),
            Expr::Null       => Ok(Value::Null),

            Expr::ColumnRef(name) => Self::resolve_column(name, row).cloned(),

            Expr::BinOp { op, lhs, rhs } => {
                let l = self.eval(lhs, row)?.as_decimal()?;
                let r = self.eval(rhs, row)?.as_decimal()?;
                let result = match op {
                    ArithOp::Add => l + r,
                    ArithOp::Sub => l - r,
                    ArithOp::Mul => l * r,
                    ArithOp::Div => {
                        if r == rust_decimal::Decimal::ZERO {
                            return Err(EvalError::DivisionByZero);
                        }
                        l / r
                    }
                };
                Ok(Value::Decimal(result))
            }

            Expr::Compare { op, lhs, rhs } => {
                let l = self.eval(lhs, row)?;
                let r = self.eval(rhs, row)?;
                Ok(Value::Bool(self.apply_cmp(op, &l, &r)))
            }

            Expr::Logical { op, lhs, rhs } => {
                let l = self.eval(lhs, row)?.as_bool()?;
                match op {
                    LogicOp::And => {
                        if !l { return Ok(Value::Bool(false)); }
                        Ok(Value::Bool(self.eval(rhs, row)?.as_bool()?))
                    }
                    LogicOp::Or => {
                        if l { return Ok(Value::Bool(true)); }
                        Ok(Value::Bool(self.eval(rhs, row)?.as_bool()?))
                    }
                }
            }

            Expr::Not(inner) => Ok(Value::Bool(!self.eval(inner, row)?.as_bool()?)),

            Expr::InList { expr, values, negated } => {
                let v = self.eval(expr, row)?;
                let found = values.iter().any(|item| {
                    self.eval(item, row)
                        .map(|r| Value::sql_eq(&v, &r))
                        .unwrap_or(false)
                });
                Ok(Value::Bool(if *negated { !found } else { found }))
            }

            Expr::Between { expr, low, high, negated } => {
                let v  = self.eval(expr, row)?;
                let lo = self.eval(low, row)?;
                let hi = self.eval(high, row)?;
                let gte = Value::partial_cmp_values(&v, &lo)
                    .map(|o| o != std::cmp::Ordering::Less)
                    .unwrap_or(false);
                let lte = Value::partial_cmp_values(&v, &hi)
                    .map(|o| o != std::cmp::Ordering::Greater)
                    .unwrap_or(false);
                let between = gte && lte;
                Ok(Value::Bool(if *negated { !between } else { between }))
            }

            Expr::IsNull { expr, negated } => {
                let v = self.eval(expr, row)?;
                let is_null = v == Value::Null;
                Ok(Value::Bool(if *negated { !is_null } else { is_null }))
            }

            Expr::Aggregate { func, expr, filter } => self.eval_aggregate(func, expr, filter),

            Expr::Assert { label, lhs, rhs, op } => {
                let result = self.eval_assert(label, lhs, rhs, op)?;
                Ok(Value::Bool(result.passed))
            }

            Expr::Sample { .. } => Ok(Value::Null),
        }
    }
}

// ─────────────────────────────────────────────
// SCRIPT RUNNER
// ─────────────────────────────────────────────

/// Evaluate every statement in a parsed script, returning one result per statement.
/// Errors are captured per-statement so a failure does not abort the rest of the script.
pub fn run_script(
    statements: &[Statement],
    datasource: &dyn EvalDataSource,
) -> Vec<StatementResult> {
    let evaluator = Evaluator::new(datasource);

    statements
        .iter()
        .map(|stmt| match &stmt.expr {
            Expr::Assert { label, lhs, rhs, op } => {
                match evaluator.eval_assert(label, lhs, rhs, op) {
                    Ok(r)  => StatementResult::Assert(r),
                    Err(e) => StatementResult::Error(e.to_string()),
                }
            }
            Expr::Sample { method, population, value_column, size, filter } => {
                match evaluator.eval_sample(method, population, value_column, size, filter) {
                    Ok(r)  => StatementResult::Sample(r),
                    Err(e) => StatementResult::Error(e.to_string()),
                }
            }
            other => match evaluator.eval(other, &Row::new()) {
                Ok(v)  => StatementResult::Value(v.to_string()),
                Err(e) => StatementResult::Error(e.to_string()),
            },
        })
        .collect()
}
