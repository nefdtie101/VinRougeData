mod aggregate;
mod assert;
mod helpers;
mod result;
mod sample;

pub use result::{AssertResult, SampleResult, StatementResult};

use crate::dsl::ast::{self, *};
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
        use helpers::{is_date_str, like_match, normalize_date};
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

            Expr::Aggregate { func, distinct, expr, filter } => self.eval_aggregate(func, *distinct, expr, filter),

            Expr::Like { expr, pattern, negated } => {
                let text = self.eval(expr, row)?.as_text();
                let pat  = self.eval(pattern, row)?.as_text();
                let matched = like_match(&text, &pat);
                Ok(Value::Bool(if *negated { !matched } else { matched }))
            }

            Expr::StringFn { func, expr } => {
                let text = self.eval(expr, row)?.as_text();
                match func {
                    ast::StringFunc::Upper  => Ok(Value::Text(text.to_uppercase())),
                    ast::StringFunc::Lower  => Ok(Value::Text(text.to_lowercase())),
                    ast::StringFunc::Trim   => Ok(Value::Text(text.trim().to_string())),
                    ast::StringFunc::Length => {
                        use rust_decimal::Decimal;
                        Ok(Value::Decimal(Decimal::from(text.chars().count())))
                    }
                }
            }

            Expr::DateFn { expr } => {
                let text = self.eval(expr, row)?.as_text();
                Ok(Value::Text(normalize_date(&text)))
            }

            Expr::Coalesce { exprs } => {
                for e in exprs {
                    let v = self.eval(e, row)?;
                    if v != Value::Null {
                        return Ok(v);
                    }
                }
                Ok(Value::Null)
            }

            Expr::NullIf { expr, compare } => {
                let v = self.eval(expr, row)?;
                let c = self.eval(compare, row)?;
                if Value::sql_eq(&v, &c) { Ok(Value::Null) } else { Ok(v) }
            }

            Expr::MathFn { func, expr, scale } => {
                use rust_decimal::Decimal;
                use crate::dsl::ast::MathFunc;
                let v = self.eval(expr, row)?.as_decimal()?;
                match func {
                    MathFunc::Abs => Ok(Value::Decimal(v.abs())),
                    MathFunc::Round => {
                        let places = match scale {
                            Some(s) => self.eval(s, row)?.as_decimal()?.try_into().unwrap_or(0i32),
                            None    => 0,
                        };
                        Ok(Value::Decimal(v.round_dp(places as u32)))
                    }
                }
            }

            Expr::Case { branches, else_expr } => {
                for (condition, result) in branches {
                    if self.eval(condition, row)?.as_bool().unwrap_or(false) {
                        return self.eval(result, row);
                    }
                }
                match else_expr {
                    Some(e) => self.eval(e, row),
                    None    => Ok(Value::Null),
                }
            }

            Expr::IsBlank { expr, negated } => {
                let v = self.eval(expr, row)?;
                let blank = v == Value::Null || v.as_text().trim().is_empty();
                Ok(Value::Bool(if *negated { !blank } else { blank }))
            }

            Expr::IsNumeric { expr, negated } => {
                let v = self.eval(expr, row)?;
                let s = v.as_text();
                let numeric = v != Value::Null
                    && rust_decimal::Decimal::from_str_exact(s.trim().replace(',', "").as_str()).is_ok();
                Ok(Value::Bool(if *negated { !numeric } else { numeric }))
            }

            Expr::IsDate { expr, negated } => {
                let v = self.eval(expr, row)?;
                let is_date_val = v != Value::Null && is_date_str(v.as_text().trim());
                Ok(Value::Bool(if *negated { !is_date_val } else { is_date_val }))
            }

            Expr::Duplicated { exprs } => {
                let table = exprs.first()
                    .and_then(|e| if let Expr::ColumnRef(n) = e.as_ref() { Some(n.as_str()) } else { None })
                    .and_then(|n| n.find('.').map(|d| &n[..d]))
                    .ok_or_else(|| EvalError::AggregateError(
                        "DUPLICATED requires table.column references".into()
                    ))?;
                let all_rows = self.datasource.rows(table)?;

                let make_key = |r: &Row| {
                    exprs.iter().map(|e| {
                        if let Expr::ColumnRef(n) = e.as_ref() {
                            Self::resolve_column(n, r).map(|v| v.to_string()).unwrap_or_default()
                        } else {
                            String::new()
                        }
                    }).collect::<Vec<_>>().join("\x00")
                };
                let current_key = make_key(row);
                let count = all_rows.iter().filter(|r| make_key(r) == current_key).count();
                Ok(Value::Bool(count > 1))
            }

            Expr::InTableCol { expr, table, column, negated } => {
                let v = self.eval(expr, row)?;
                let all_rows = self.datasource.rows(table.as_str())?;
                let col_lc = column.to_lowercase();
                let found = all_rows.iter().any(|r| {
                    let col_val = r.get(&col_lc)
                        .or_else(|| r.iter().find(|(k, _)| k.eq_ignore_ascii_case(&col_lc)).map(|(_, v)| v));
                    col_val.map(|cv| Value::sql_eq(&v, cv)).unwrap_or(false)
                });
                Ok(Value::Bool(if *negated { !found } else { found }))
            }

            // RELATION is metadata only — evaluates as a no-op
            Expr::RelationDecl { .. } => Ok(Value::Bool(true)),

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
            Expr::RelationDecl { from_table, from_col, to_table, to_col } => {
                StatementResult::Relation {
                    from: format!("{from_table}.{from_col}"),
                    to:   format!("{to_table}.{to_col}"),
                }
            }
            other => match evaluator.eval(other, &Row::new()) {
                Ok(v)  => StatementResult::Value(v.to_string()),
                Err(e) => StatementResult::Error(e.to_string()),
            },
        })
        .collect()
}
