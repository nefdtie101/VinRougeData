use std::collections::BTreeMap;

use crate::dsl::ast::{AggFunc, Expr};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::result::ChartResult;
use super::Evaluator;

impl<'ds> Evaluator<'ds> {
    pub(super) fn eval_chart(
        &self,
        chart_type: &str,
        aggregate: &Expr,
        dimension: &Expr,
        label: Option<String>,
    ) -> EvalResult<ChartResult> {
        let dim_col = match dimension {
            Expr::ColumnRef(name) => Some(name.clone()),
            _ => None,
        };

        let dim_table = dim_col.as_ref()
            .and_then(|n| n.find('.').map(|d| &n[..d]))
            .or_else(|| find_table_in_aggregate(aggregate))
            .ok_or_else(|| EvalError::AggregateError(
                "chart dimension must use table.column notation, or the aggregate must reference a table".to_string()
            ))?;

        let all_rows = self.datasource.rows(dim_table)?;

        // Group rows by dimension value
        let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
        for row in all_rows {
            let dim_val = if let Some(ref name) = dim_col {
                Self::resolve_column(name, row)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            } else {
                self.eval(dimension, row)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            };
            groups.entry(dim_val).or_default().push(row);
        }

        // Evaluate aggregate per group
        let mut labels = Vec::new();
        let mut values = Vec::new();

        for (dim_val, group_rows) in groups {
            let agg_val = self.eval_aggregate_over_rows(aggregate, &group_rows)?;
            labels.push(dim_val);
            values.push(agg_val.to_string());
        }

        Ok(ChartResult {
            label,
            chart_type: chart_type.to_string(),
            labels,
            values,
        })
    }

    /// Evaluate an aggregate expression over a specific set of rows.
    fn eval_aggregate_over_rows(&self, expr: &Expr, rows: &[&Row]) -> EvalResult<Value> {
        let (func, distinct, inner_expr, filter) = match expr {
            Expr::Aggregate {
                func,
                distinct,
                expr,
                filter,
            } => (func, *distinct, expr.as_ref(), filter.as_ref()),
            other => {
                return Err(EvalError::AggregateError(format!(
                    "expected aggregate expression, got {other:?}"
                )))
            }
        };

        let filtered: Vec<&&Row> = rows
            .iter()
            .filter(|row| {
                filter
                    .map(|f| {
                        self.eval(f, row)
                            .map(|v| v.as_bool().unwrap_or(false))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .collect();

        let values: Vec<Value> = filtered
            .iter()
            .map(|row| self.eval(inner_expr, row))
            .collect::<EvalResult<Vec<_>>>()?;

        let non_null: Vec<Value> = values.into_iter().filter(|v| *v != Value::Null).collect();

        let effective: Vec<&Value> = if distinct {
            let mut seen = std::collections::HashSet::new();
            non_null
                .iter()
                .filter(|v| seen.insert(v.to_string()))
                .collect()
        } else {
            non_null.iter().collect()
        };

        use rust_decimal::Decimal;
        match func {
            AggFunc::Count => Ok(Value::Decimal(Decimal::from(effective.len()))),
            AggFunc::Sum => {
                let sum = effective
                    .iter()
                    .map(|v| v.as_decimal())
                    .collect::<EvalResult<Vec<_>>>()?
                    .into_iter()
                    .fold(Decimal::ZERO, |acc, d| acc + d);
                Ok(Value::Decimal(sum))
            }
            AggFunc::Avg => {
                if effective.is_empty() {
                    return Ok(Value::Null);
                }
                let sum = effective
                    .iter()
                    .map(|v| v.as_decimal())
                    .collect::<EvalResult<Vec<_>>>()?
                    .into_iter()
                    .fold(Decimal::ZERO, |acc, d| acc + d);
                Ok(Value::Decimal(sum / Decimal::from(effective.len())))
            }
            AggFunc::Min => {
                if effective.is_empty() {
                    return Ok(Value::Null);
                }
                let mut min = effective[0].clone();
                for v in &effective[1..] {
                    if Value::partial_cmp_values(v, &min) == Some(std::cmp::Ordering::Less) {
                        min = (*v).clone();
                    }
                }
                Ok(min)
            }
            AggFunc::Max => {
                if effective.is_empty() {
                    return Ok(Value::Null);
                }
                let mut max = effective[0].clone();
                for v in &effective[1..] {
                    if Value::partial_cmp_values(v, &max) == Some(std::cmp::Ordering::Greater) {
                        max = (*v).clone();
                    }
                }
                Ok(max)
            }
        }
    }
}

/// Find the first table prefix inside an aggregate expression or its filter.
fn find_table_in_aggregate(agg: &Expr) -> Option<&str> {
    let (expr, filter) = match agg {
        Expr::Aggregate { expr, filter, .. } => (expr.as_ref(), filter.as_deref()),
        _ => return None,
    };
    find_table_in_expr(expr).or_else(|| filter.and_then(find_table_in_expr))
}

/// Recursively search for a `table.column` reference in an expression.
fn find_table_in_expr(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::ColumnRef(name) => name.find('.').map(|d| &name[..d]),
        Expr::BinOp { lhs, rhs, .. } => find_table_in_expr(lhs).or_else(|| find_table_in_expr(rhs)),
        Expr::Compare { lhs, rhs, .. } => {
            find_table_in_expr(lhs).or_else(|| find_table_in_expr(rhs))
        }
        Expr::Logical { lhs, rhs, .. } => {
            find_table_in_expr(lhs).or_else(|| find_table_in_expr(rhs))
        }
        Expr::Not(inner) => find_table_in_expr(inner),
        Expr::Case {
            branches,
            else_expr,
        } => branches
            .iter()
            .find_map(|(c, r)| find_table_in_expr(c).or_else(|| find_table_in_expr(r)))
            .or_else(|| else_expr.as_deref().and_then(find_table_in_expr)),
        Expr::Coalesce { exprs } => exprs.iter().find_map(|e| find_table_in_expr(e)),
        Expr::NullIf { expr, compare } => {
            find_table_in_expr(expr).or_else(|| find_table_in_expr(compare))
        }
        Expr::MathFn { expr, scale, .. } => {
            find_table_in_expr(expr).or_else(|| scale.as_deref().and_then(find_table_in_expr))
        }
        Expr::StringFn { expr, .. } => find_table_in_expr(expr),
        Expr::SubStr {
            expr,
            start,
            length,
        } => find_table_in_expr(expr)
            .or_else(|| find_table_in_expr(start))
            .or_else(|| length.as_deref().and_then(find_table_in_expr)),
        Expr::Concat { exprs } => exprs.iter().find_map(|e| find_table_in_expr(e)),
        Expr::DateFn { expr } => find_table_in_expr(expr),
        Expr::IsNull { expr, .. } => find_table_in_expr(expr),
        Expr::IsBlank { expr, .. } => find_table_in_expr(expr),
        Expr::IsNumeric { expr, .. } => find_table_in_expr(expr),
        Expr::IsDate { expr, .. } => find_table_in_expr(expr),
        Expr::SaIdValid { expr } => find_table_in_expr(expr),
        Expr::Duplicated { exprs } => exprs.iter().find_map(|e| find_table_in_expr(e)),
        Expr::Like { expr, pattern, .. } => {
            find_table_in_expr(expr).or_else(|| find_table_in_expr(pattern))
        }
        Expr::InList { expr, values, .. } => {
            find_table_in_expr(expr).or_else(|| values.iter().find_map(find_table_in_expr))
        }
        Expr::Between {
            expr, low, high, ..
        } => find_table_in_expr(expr)
            .or_else(|| find_table_in_expr(low))
            .or_else(|| find_table_in_expr(high)),
        _ => None,
    }
}
