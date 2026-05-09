use std::collections::HashMap;
use std::sync::Mutex;

use rust_decimal::Decimal;

use vinrouge::dsl::{
    expr_to_sql, AggFunc, EvalDataSource, EvalError, EvalResult, Expr, Row, SchemaColumn,
    SchemaTable, Value,
};

// ─────────────────────────────────────────────
// DuckDB data source
// ─────────────────────────────────────────────

/// An [`EvalDataSource`] backed by an in-memory DuckDB database.
///
/// Aggregate functions (`SUM`, `COUNT`, `AVG`, `MIN`, `MAX`) are pushed
/// down to DuckDB's vectorised engine and never materialise rows in Rust.
/// Row-level operations that need a full `&[Row]` slice (sampling,
/// `DUPLICATED`, `IN table.col`) still use the in-memory row cache.
pub struct DuckDbDataSource {
    conn: Mutex<duckdb::Connection>,
    rows_cache: HashMap<String, Vec<Row>>,
    col_types: HashMap<String, HashMap<String, ColType>>,
}

#[derive(Clone, Copy)]
enum ColType {
    Numeric,
    Text,
}

impl DuckDbDataSource {
    pub fn new() -> Result<Self, String> {
        let conn = duckdb::Connection::open_in_memory().map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
            rows_cache: HashMap::new(),
            col_types: HashMap::new(),
        })
    }

    /// Load a table into both DuckDB and the in-memory row cache.
    pub fn insert_table(&mut self, name: impl Into<String>, rows: Vec<Row>) -> Result<(), String> {
        let name: String = name.into();

        if rows.is_empty() {
            self.rows_cache.insert(name, rows);
            return Ok(());
        }

        // Collect column names in sorted order for deterministic DDL.
        let mut col_names: Vec<String> = rows[0].keys().cloned().collect();
        col_names.sort();

        // Infer types: DOUBLE when every non-null value in the column is Decimal.
        let mut table_col_types: HashMap<String, ColType> = HashMap::new();
        for col in &col_names {
            let all_numeric = rows.iter().all(|row| {
                matches!(
                    row.get(col),
                    Some(Value::Decimal(_)) | Some(Value::Null) | None
                )
            });
            table_col_types.insert(
                col.clone(),
                if all_numeric {
                    ColType::Numeric
                } else {
                    ColType::Text
                },
            );
        }

        // ── Create the DuckDB table ───────────────────────────────────────────
        let cols_ddl: String = col_names
            .iter()
            .map(|n| {
                let sql_type = match table_col_types[n] {
                    ColType::Numeric => "DOUBLE",
                    ColType::Text => "VARCHAR",
                };
                format!("\"{}\" {}", n.replace('"', "\"\""), sql_type)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(&format!(
            "CREATE TABLE \"{}\" ({});",
            name.replace('"', "\"\""),
            cols_ddl
        ))
        .map_err(|e| e.to_string())?;

        // ── Insert rows in batches inside a transaction ───────────────────────
        // Build VALUES rows as SQL literals and flush in groups of 2 000.
        const BATCH: usize = 2_000;
        let quoted_table = format!("\"{}\"", name.replace('"', "\"\""));
        let quoted_cols: String = col_names
            .iter()
            .map(|n| format!("\"{}\"", n.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        for chunk in rows.chunks(BATCH) {
            let values_clause: String = chunk
                .iter()
                .map(|row| {
                    let cells: String = col_names
                        .iter()
                        .map(|col| value_to_sql_literal(row.get(col), table_col_types[col]))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("({cells})")
                })
                .collect::<Vec<_>>()
                .join(", ");

            conn.execute_batch(&format!(
                "INSERT INTO {quoted_table} ({quoted_cols}) VALUES {values_clause};"
            ))
            .map_err(|e| e.to_string())?;
        }

        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;

        self.col_types.insert(name.clone(), table_col_types);
        self.rows_cache.insert(name, rows);
        Ok(())
    }
}

impl EvalDataSource for DuckDbDataSource {
    fn rows(&self, table: &str) -> Result<&[Row], EvalError> {
        self.rows_cache
            .get(table)
            .map(|v| v.as_slice())
            .ok_or_else(|| EvalError::UnknownTable(table.to_string()))
    }

    fn schema_tables(&self) -> Vec<SchemaTable> {
        let mut tables: Vec<SchemaTable> = self
            .col_types
            .iter()
            .map(|(name, cols)| {
                let mut col_names: Vec<String> = cols.keys().cloned().collect();
                col_names.sort();
                let columns = col_names
                    .into_iter()
                    .map(|c| SchemaColumn {
                        col_type: match cols[&c] {
                            ColType::Numeric => "numeric",
                            ColType::Text => "text",
                        }
                        .to_string(),
                        name: c,
                    })
                    .collect();
                let row_count = self.rows_cache.get(name).map(|r| r.len()).unwrap_or(0);
                SchemaTable {
                    name: name.clone(),
                    columns,
                    row_count,
                }
            })
            .collect();
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        tables
    }

    fn try_aggregate(
        &self,
        func: &AggFunc,
        distinct: bool,
        table: &str,
        col: &str,
        filter: &Option<Box<Expr>>,
    ) -> Option<EvalResult<Value>> {
        // Only push down if the column exists and is numeric for SUM/AVG.
        // COUNT and MIN/MAX work on any type.
        let col_types = self.col_types.get(table)?;
        let col_type = col_types.get(col).copied();
        match func {
            AggFunc::Sum | AggFunc::Avg => {
                if !matches!(col_type, Some(ColType::Numeric)) {
                    return None;
                }
            }
            _ => {}
        }

        // Compile the filter expression to SQL.
        let where_clause = match filter {
            Some(f) => match expr_to_sql(f) {
                Some(sql) => format!(" WHERE {sql}"),
                None => return None, // unsupported filter — fall back to row iteration
            },
            None => String::new(),
        };

        let distinct_kw = if distinct { "DISTINCT " } else { "" };
        let quoted_col = format!("\"{}\"", col.replace('"', "\"\""));
        let quoted_table = format!("\"{}\"", table.replace('"', "\"\""));

        let query = match func {
            AggFunc::Sum => {
                format!("SELECT SUM({distinct_kw}{quoted_col})   FROM {quoted_table}{where_clause}")
            }
            AggFunc::Avg => {
                format!("SELECT AVG({distinct_kw}{quoted_col})   FROM {quoted_table}{where_clause}")
            }
            AggFunc::Count => {
                format!("SELECT COUNT({distinct_kw}{quoted_col}) FROM {quoted_table}{where_clause}")
            }
            AggFunc::Min => {
                format!("SELECT MIN({quoted_col})                FROM {quoted_table}{where_clause}")
            }
            AggFunc::Max => {
                format!("SELECT MAX({quoted_col})                FROM {quoted_table}{where_clause}")
            }
        };

        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => {
                return Some(Err(EvalError::AggregateError(
                    "duckdb lock poisoned".into(),
                )))
            }
        };

        let result = conn.query_row(&query, [], |row| row.get::<_, Option<f64>>(0));

        Some(match result {
            Ok(Some(val)) => {
                // Round-trip via string to avoid f64 imprecision creeping into Decimal.
                let d = format!("{val:.10}")
                    .parse::<Decimal>()
                    .unwrap_or_else(|_| val.to_string().parse::<Decimal>().unwrap_or_default());
                Ok(Value::Decimal(d))
            }
            Ok(None) => Ok(Value::Null),
            Err(e) => Err(EvalError::AggregateError(e.to_string())),
        })
    }
}

// ─────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────

fn value_to_sql_literal(val: Option<&Value>, col_type: ColType) -> String {
    match val {
        None | Some(Value::Null) => "NULL".to_string(),
        Some(Value::Decimal(d)) => d.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Text(s)) => match col_type {
            ColType::Numeric => s
                .trim()
                .replace(',', "")
                .parse::<f64>()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "NULL".to_string()),
            ColType::Text => format!("'{}'", s.replace('\'', "''")),
        },
        Some(Value::List(_)) => "NULL".to_string(),
    }
}
