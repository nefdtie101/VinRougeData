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
