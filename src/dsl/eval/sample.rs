use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::dsl::ast::{Expr, SampleMethod, SampleSize};
use crate::dsl::value::{EvalError, EvalResult, Row, Value};

use super::result::SampleResult;
use super::Evaluator;

impl<'ds> Evaluator<'ds> {
    pub fn eval_sample(
        &self,
        method: &SampleMethod,
        population: &str,
        value_column: &str,
        size: &SampleSize,
        filter: &Option<Box<Expr>>,
    ) -> EvalResult<SampleResult> {
        let all_rows = self.datasource.rows(population)?;

        let filtered: Vec<&Row> = all_rows
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

        let pop_size = filtered.len();
        if pop_size == 0 {
            return Err(EvalError::SampleError(
                "population is empty after filtering".to_string(),
            ));
        }

        // Bare column name (strip table prefix)
        let col = if let Some(dot) = value_column.find('.') {
            &value_column[dot + 1..]
        } else {
            value_column
        };

        let n = match size {
            SampleSize::Count(d) => {
                let n = d.to_usize().ok_or_else(|| {
                    EvalError::SampleError(format!("invalid sample count: {d}"))
                })?;
                n.min(pop_size)
            }
            SampleSize::Percent(d) => {
                let pct = d.to_f64().unwrap_or(0.0) / 100.0;
                let n = (pct * pop_size as f64).ceil() as usize;
                n.min(pop_size).max(1)
            }
        };

        let selected_rows: Vec<Row> = match method {
            SampleMethod::Mus        => self.sample_mus(&filtered, col, n)?,
            SampleMethod::Random     => self.sample_random(&filtered, n),
            SampleMethod::Systematic => self.sample_systematic(&filtered, n),
            SampleMethod::Stratified => self.sample_stratified(&filtered, col, n),
        };

        let selected = selected_rows
            .into_iter()
            .map(|row| row.into_iter().map(|(k, v)| (k, v.to_string())).collect())
            .collect();

        Ok(SampleResult {
            method: format!("{method:?}"),
            population_table: population.to_string(),
            population_size: pop_size,
            selected,
        })
    }

    // ── Sampling algorithms ───────────────────────────────────────────────────

    /// Monetary Unit Sampling — each currency unit has equal selection probability.
    pub(super) fn sample_mus(&self, rows: &[&Row], col: &str, n: usize) -> EvalResult<Vec<Row>> {
        let decimals: Vec<Decimal> = rows
            .iter()
            .map(|row| col_decimal(row, col))
            .collect();

        let total: Decimal = decimals.iter().filter(|d| **d > Decimal::ZERO).sum();

        if total == Decimal::ZERO {
            return Err(EvalError::SampleError(
                "MUS requires a positive total value in the value column".to_string(),
            ));
        }

        let interval = total / Decimal::from(n);

        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::SmallRng::from_entropy();
        let r: f64 = rng.gen::<f64>();
        let random_start =
            Decimal::from_f64_retain(r * interval.to_f64().unwrap_or(1.0))
                .unwrap_or(Decimal::ZERO);

        let mut selected: Vec<Row> = Vec::with_capacity(n);
        let mut cumulative = Decimal::ZERO;

        'outer: for (row, &val) in rows.iter().zip(decimals.iter()) {
            if val <= Decimal::ZERO {
                continue;
            }
            cumulative += val;
            loop {
                let threshold = random_start + interval * Decimal::from(selected.len());
                if threshold < cumulative {
                    selected.push((*row).clone());
                    if selected.len() >= n {
                        break 'outer;
                    }
                } else {
                    break;
                }
            }
        }

        // Top up with systematic if skewed data left gaps
        if selected.len() < n {
            let needed = n - selected.len();
            for row in self.sample_systematic(rows, needed) {
                if selected.len() >= n { break; }
                selected.push(row);
            }
        }

        Ok(selected)
    }

    /// Simple random sample without replacement.
    pub(super) fn sample_random(&self, rows: &[&Row], n: usize) -> Vec<Row> {
        use rand::{seq::SliceRandom, SeedableRng};
        let mut rng = rand::rngs::SmallRng::from_entropy();
        let mut indices: Vec<usize> = (0..rows.len()).collect();
        indices.shuffle(&mut rng);
        indices[..n].iter().map(|&i| rows[i].clone()).collect()
    }

    /// Systematic sample — every k-th item from a random start.
    pub(super) fn sample_systematic(&self, rows: &[&Row], n: usize) -> Vec<Row> {
        let len = rows.len();
        if n >= len {
            return rows.iter().map(|r| (*r).clone()).collect();
        }
        let step = (len / n).max(1);
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::SmallRng::from_entropy();
        let start: usize = rng.gen_range(0..step);
        (0..n)
            .map(|i| rows[(start + i * step).min(len - 1)].clone())
            .collect()
    }

    /// Stratified sample — top half by value + random from remainder (ISA 530).
    pub(super) fn sample_stratified(&self, rows: &[&Row], col: &str, n: usize) -> Vec<Row> {
        let mut sorted: Vec<(&Row, Decimal)> = rows
            .iter()
            .map(|row| (*row, col_decimal(row, col)))
            .collect();

        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_n = (n / 2).max(1);
        let rest_n = n - top_n;

        let top: Vec<Row> = sorted[..top_n.min(sorted.len())]
            .iter()
            .map(|(r, _)| (*r).clone())
            .collect();

        let remainder: Vec<&Row> = sorted[top_n.min(sorted.len())..]
            .iter()
            .map(|(r, _)| *r)
            .collect();

        let mut rest = self.sample_random(&remainder, rest_n.min(remainder.len()));
        let mut result = top;
        result.append(&mut rest);
        result
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Look up a column value as Decimal (case-insensitive, defaults to zero).
fn col_decimal(row: &Row, col: &str) -> Decimal {
    row.get(col)
        .or_else(|| {
            row.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(col))
                .map(|(_, v)| v)
        })
        .and_then(|v| v.as_decimal().ok())
        .unwrap_or(Decimal::ZERO)
}
