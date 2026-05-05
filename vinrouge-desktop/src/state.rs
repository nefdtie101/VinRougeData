use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vinrouge::analysis::Workflow;
use vinrouge::schema::{Relationship, Table};

// ── Shared state managed by Tauri ─────────────────────────────────────────────

pub struct OllamaState(pub Mutex<Option<std::process::Child>>);
pub struct ProjectsState(pub Mutex<Option<PathBuf>>);

/// Caches the DuckDB datasource so SQLite is only read once per import session.
/// Wrapped in Arc so it can be cloned cheaply into spawn_blocking closures.
pub struct DslCacheState(pub Arc<Mutex<DslCache>>);

pub struct DslCache {
    /// The project dir the cached datasource was built for.
    pub project_dir: Option<PathBuf>,
    pub datasource: Option<Arc<crate::duckdb_source::DuckDbDataSource>>,
}

impl Default for DslCacheState {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(DslCache {
            project_dir: None,
            datasource: None,
        })))
    }
}

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AnalysisOutput {
    pub tables: Vec<Table>,
    pub relationships: Vec<Relationship>,
    pub workflows: Vec<Workflow>,
}
