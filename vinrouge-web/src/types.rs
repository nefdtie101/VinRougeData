use vinrouge::analysis::Workflow;
use vinrouge::schema::{Relationship, Table};

// ── Domain ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Deserialize)]
pub struct AnalysisResult {
    pub tables: Vec<Table>,
    pub relationships: Vec<Relationship>,
    pub workflows: Vec<Workflow>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ProjectFile {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub uploaded_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct AiMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Control {
    pub id: String,
    pub process_id: String,
    pub control_ref: String,
    pub control_objective: String,
    pub control_description: String,
    pub test_procedure: String,
    pub risk_level: String,
    #[serde(default)]
    pub sop_gap: bool,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuditProcessWithControls {
    pub id: String,
    pub sop_file_id: String,
    pub process_name: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
    pub controls: Vec<Control>,
    #[serde(default)]
    pub audit_prompt: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PbcItem {
    pub id: String,
    pub control_id: String,
    pub control_ref: String,
    pub name: String,
    pub item_type: String,
    pub table_name: Option<String>,
    pub fields: Vec<String>,
    pub purpose: String,
    pub scope_format: String,
    pub approved: bool,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct PbcGroup {
    pub control_id: String,
    pub control_ref: String,
    pub title: String,
    pub process_name: String,
    pub items: Vec<PbcItem>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SessionImport {
    pub id: String,
    pub file_id: Option<String>,
    pub source_type: String,
    pub source_name: String,
    pub row_count: usize,
    pub mappings: Vec<(String, String)>,
    pub imported_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SessionSchema {
    pub import_id: String,
    pub source_type: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub row_count: usize,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct RelCandidate {
    pub left_import_id: String,
    pub left_table: String,
    pub left_col: String,
    pub right_import_id: String,
    pub right_table: String,
    pub right_col: String,
    pub confidence: u8,
    pub overlap_count: usize,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct JoinSpec {
    pub left_import_id: String,
    pub left_col: String,
    pub right_import_id: String,
    pub right_col: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DslScript {
    pub id: String,
    pub control_id: String,
    pub control_ref: String,
    pub label: String,
    pub script_text: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TestResult {
    pub id: String,
    pub script_id: String,
    pub results: Vec<serde_json::Value>,
    pub passed_count: i64,
    pub failed_count: i64,
    pub error_count: i64,
    pub run_at: String,
}
