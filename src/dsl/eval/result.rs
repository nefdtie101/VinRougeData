use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertResult {
    pub label: Option<String>,
    pub passed: bool,
    pub lhs_value: String,
    pub rhs_value: String,
    pub op: String,
    /// `table.column` extracted from the LHS, used by the frontend to load chart data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_col: Option<String>,
    /// Rows that failed the assertion, present only when `SHOW FAILURES IN TABLE` was specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_rows: Option<Vec<HashMap<String, String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleResult {
    pub method: String,
    pub population_table: String,
    pub population_size: usize,
    pub selected: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartResult {
    pub label: Option<String>,
    pub chart_type: String,
    pub labels: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionResult {
    pub title: String,
    pub results: Vec<StatementResult>,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaColumn {
    pub name: String,
    pub col_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementResult {
    Value(String),
    Assert(AssertResult),
    Sample(SampleResult),
    /// Produced by a RELATION declaration — carries the FK mapping as strings.
    Relation {
        from: String,
        to: String,
    },
    Chart(ChartResult),

    Section(SectionResult),
    Schema(Vec<SchemaTable>),
    Error(String),
}
