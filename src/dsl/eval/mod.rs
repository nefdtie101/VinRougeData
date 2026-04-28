mod aggregate;
mod assert;
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

            Expr::Assert { label, lhs, rhs, op } => {
                let result = self.eval_assert(label, lhs, rhs, op)?;
                Ok(Value::Bool(result.passed))
            }

            Expr::Sample { .. } => Ok(Value::Null),
        }
    }
}

// ─────────────────────────────────────────────
// HELPERS
// ─────────────────────────────────────────────

/// SQL LIKE matching: `%` = any sequence, `_` = any single char, case-insensitive.
fn like_match(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (tn, pn) = (t.len(), p.len());
    // DP table: dp[i][j] = t[..i] matches p[..j]
    let mut dp = vec![vec![false; pn + 1]; tn + 1];
    dp[0][0] = true;
    for j in 1..=pn {
        if p[j - 1] == '%' {
            dp[0][j] = dp[0][j - 1];
        }
    }
    for i in 1..=tn {
        for j in 1..=pn {
            dp[i][j] = match p[j - 1] {
                '%' => dp[i - 1][j] || dp[i][j - 1],
                '_' => dp[i - 1][j - 1],
                c   => dp[i - 1][j - 1] && t[i - 1].to_ascii_lowercase() == c.to_ascii_lowercase(),
            };
        }
    }
    dp[tn][pn]
}

/// Normalise common date formats to ISO 8601 (YYYY-MM-DD) for consistent string ordering.
/// Recognises: YYYY-MM-DD, YYYY/MM/DD, DD/MM/YYYY, DD-MM-YYYY, MM/DD/YYYY.
/// Falls back to the original string if unrecognised.
fn normalize_date(s: &str) -> String {
    let s = s.trim();
    // Already ISO: YYYY-MM-DD or YYYY/MM/DD
    if s.len() == 10 {
        let sep = s.as_bytes()[4];
        if sep == b'-' || sep == b'/' {
            let y = &s[0..4];
            let m = &s[5..7];
            let d = &s[8..10];
            if y.chars().all(|c| c.is_ascii_digit()) {
                return format!("{y}-{m}-{d}");
            }
        }
        // DD/MM/YYYY or DD-MM-YYYY or MM/DD/YYYY — all have sep at positions 2 and 5
        let sep2 = s.as_bytes()[2];
        if sep2 == b'/' || sep2 == b'-' {
            let a = &s[0..2];
            let b = &s[3..5];
            let y = &s[6..10];
            if y.chars().all(|c| c.is_ascii_digit())
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
            {
                // Heuristic: if first part > 12 it must be DD/MM/YYYY
                let first: u32 = a.parse().unwrap_or(0);
                let (month, day) = if first > 12 { (b, a) } else { (a, b) };
                return format!("{y}-{month}-{day}");
            }
        }
    }
    s.to_string()
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
