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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleResult {
    pub method: String,
    pub population_table: String,
    pub population_size: usize,
    pub selected: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementResult {
    Value(String),
    Assert(AssertResult),
    Sample(SampleResult),
    Error(String),
}
