use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use vinrouge::dsl::InMemoryDataSource;
use vinrouge::projects::{DslScript, Project};

#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadedProject {
    pub project: Project,
    pub project_dir: PathBuf,
}

pub struct AppState {
    pub projects: RwLock<HashMap<String, LoadedProject>>,
    /// Scripts loaded from the .vrd file at startup — keyed by project_id.
    pub scripts: RwLock<HashMap<String, Vec<DslScript>>>,
    pub datasources: RwLock<HashMap<String, Arc<InMemoryDataSource>>>,
    /// Keyed by "{project_id}/{script_id}" → pre-computed JSON results.
    pub results: RwLock<HashMap<String, Vec<serde_json::Value>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            projects: RwLock::new(HashMap::new()),
            scripts: RwLock::new(HashMap::new()),
            datasources: RwLock::new(HashMap::new()),
            results: RwLock::new(HashMap::new()),
        }
    }
}
