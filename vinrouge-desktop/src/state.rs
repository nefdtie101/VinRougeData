use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use vinrouge::analysis::Workflow;
use vinrouge::schema::{Relationship, Table};

// ── Shared state managed by Tauri ─────────────────────────────────────────────

pub struct OllamaState(pub Mutex<Option<std::process::Child>>);
pub struct ProjectsState(pub Mutex<Option<PathBuf>>);

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AnalysisOutput {
    pub tables: Vec<Table>,
    pub relationships: Vec<Relationship>,
    pub workflows: Vec<Workflow>,
}
