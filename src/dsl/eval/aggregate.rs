use rust_decimal::Decimal;

use crate::dsl::ast::{AggFunc, Expr};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::Evaluator;

impl<'ds> Evaluator<'ds> {
    pub(super) fn eval_aggregate(
        &self,
        func: &AggFunc,
        distinct: bool,
        expr: &Expr,
        filter: &Option<Box<Expr>>,
    ) -> EvalResult<Value> {
        let table = Self::table_from_expr(expr).ok_or_else(|| {
            EvalError::AggregateError(
                "aggregate inner expression must be a table.column reference".to_string(),
            )
        })?;

        let all_rows = self.datasource.rows(table)?;

        let rows: Vec<&Row> = all_rows
            .iter()
            .filter(|row| {
                filter
                    .as_ref()
                    .map(|f| {
                        self.eval(f, row)
                            .map(|v| v.as_bool().unwrap_or(false))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .collect();

        let values: Vec<Value> = rows
            .iter()
            .map(|row| self.eval(expr, row))
            .collect::<EvalResult<Vec<_>>>()?;

        let non_null: Vec<Value> = values.into_iter().filter(|v| *v != Value::Null).collect();

        // For DISTINCT aggregates deduplicate by string representation
        let effective: Vec<&Value> = if distinct {
            let mut seen = std::collections::HashSet::new();
            non_null.iter().filter(|v| seen.insert(v.to_string())).collect()
        } else {
            non_null.iter().collect()
        };

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

    /// Extract the table prefix from a ColumnRef, e.g. `"invoices.amount"` → `"invoices"`.
    pub(super) fn table_from_expr(expr: &Expr) -> Option<&str> {
        if let Expr::ColumnRef(name) = expr {
            if let Some(dot) = name.find('.') {
                return Some(&name[..dot]);
            }
        }
        None
    }
}
