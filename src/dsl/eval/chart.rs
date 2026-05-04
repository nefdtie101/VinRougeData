use std::collections::BTreeMap;

use crate::dsl::ast::{AggFunc, ChartDef, Expr};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::Evaluator;
use super::result::{ChartResult, ScreenResult};

impl<'ds> Evaluator<'ds> {
    pub(super) fn eval_chart(
        &self,
        chart_type: &str,
        aggregate: &Expr,
        dimension: &Expr,
        label: Option<String>,
    ) -> EvalResult<ChartResult> {
        // Dimension must be a ColumnRef
        let dim_col = match dimension {
            Expr::ColumnRef(name) => name,
            other => return Err(EvalError::AggregateError(
                format!("chart dimension must be a column reference, got {other:?}")
            )),
        };

        let dim_table = dim_col.find('.')
            .map(|d| &dim_col[..d])
            .ok_or_else(|| EvalError::AggregateError(
                format!("chart dimension must use table.column notation, got '{dim_col}'")
            ))?;

        let all_rows = self.datasource.rows(dim_table)?;

        // Group rows by dimension value
        let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
        for row in all_rows {
            let dim_val = Self::resolve_column(dim_col, row)
                .map(|v| v.to_string())
                .unwrap_or_default();
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

    pub(super) fn eval_screen(
        &self,
        title: &str,
        charts: &[ChartDef],
    ) -> EvalResult<ScreenResult> {
        let mut results = Vec::with_capacity(charts.len());
        for chart in charts {
            let result = self.eval_chart(
                &chart.chart_type,
                &chart.aggregate,
                &chart.dimension,
                chart.label.clone(),
            )?;
            results.push(result);
        }
        Ok(ScreenResult {
            title: title.to_string(),
            charts: results,
        })
    }

    /// Evaluate an aggregate expression over a specific set of rows.
    fn eval_aggregate_over_rows(
        &self,
        expr: &Expr,
        rows: &[&Row],
    ) -> EvalResult<Value> {
        let (func, distinct, inner_expr, filter) = match expr {
            Expr::Aggregate { func, distinct, expr, filter } => (func, *distinct, expr.as_ref(), filter.as_ref()),
            other => return Err(EvalError::AggregateError(
                format!("expected aggregate expression, got {other:?}")
            )),
        };

        let filtered: Vec<&&Row> = rows.iter()
            .filter(|row| {
                filter.map(|f| {
                    self.eval(f, row)
                        .map(|v| v.as_bool().unwrap_or(false))
                        .unwrap_or(false)
                }).unwrap_or(true)
            })
            .collect();

        let values: Vec<Value> = filtered.iter()
            .map(|row| self.eval(inner_expr, row))
            .collect::<EvalResult<Vec<_>>>()?;

        let non_null: Vec<Value> = values.into_iter().filter(|v| *v != Value::Null).collect();

        let effective: Vec<&Value> = if distinct {
            let mut seen = std::collections::HashSet::new();
            non_null.iter().filter(|v| seen.insert(v.to_string())).collect()
        } else {
            non_null.iter().collect()
        };

        use rust_decimal::Decimal;
        match func {
            AggFunc::Count => Ok(Value::Decimal(Decimal::from(effective.len()))),
            AggFunc::Sum => {
                let sum = effective.iter()
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
                let sum = effective.iter()
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
