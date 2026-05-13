use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vinrouge::analysis::Workflow;
use vinrouge::schema::{Relationship, Table};

// ── Shared state managed by Tauri ─────────────────────────────────────────────

pub struct OllamaState(pub Mutex<Option<std::process::Child>>);

#[derive(Clone)]
pub struct ActiveProject {
    pub dir: PathBuf,
    /// The .vrd file this project is linked to — the on-disk source of truth.
    /// Local storage (SQLite) is only a working cache.
    pub vrd: Option<PathBuf>,
}

/// Holds the currently active project and its linked .vrd path.
/// All mutating commands call `sync_vrd()` to keep the .vrd up to date.
pub struct ProjectsState(pub Mutex<Option<ActiveProject>>);

impl ProjectsState {
    /// Returns the active project directory, or an error if none is open.
    pub fn dir(&self) -> Result<PathBuf, String> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .map(|a| a.dir.clone())
            .ok_or_else(|| "No active project".into())
    }

    /// Set (or replace) the active project and its optional .vrd link.
    pub fn activate(&self, dir: PathBuf, vrd: Option<PathBuf>) {
        *self.0.lock().unwrap() = Some(ActiveProject { dir, vrd });
    }

    /// Link the active project to a specific .vrd file.
    pub fn set_vrd(&self, vrd: PathBuf) {
        if let Some(a) = self.0.lock().unwrap().as_mut() {
            a.vrd = Some(vrd);
        }
    }

    pub fn get_vrd(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().as_ref().and_then(|a| a.vrd.clone())
    }

    /// Re-pack the active project into its linked .vrd file.
    ///
    /// If no .vrd is linked yet, one is created automatically at
    /// `~/VinRouge/vrd/<ProjectName>.vrd` and linked for future calls.
    /// Returns `Ok(())` silently when no project is active.
    pub fn sync_vrd(&self) -> Result<(), String> {
        let (dir, vrd_path) = {
            let mut guard = self.0.lock().unwrap();
            let active = match guard.as_mut() {
                Some(a) => a,
                None => return Ok(()),
            };
            if active.vrd.is_none() {
                let project = vinrouge::projects::load_project(&active.dir)?;
                let home = vinrouge::projects::vinrouge_home()?;
                let vrd_dir = home.join("vrd");
                std::fs::create_dir_all(&vrd_dir).map_err(|e| e.to_string())?;
                let safe_name: String = project
                    .name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>()
                    .trim()
                    .to_string();
                active.vrd = Some(vrd_dir.join(format!("{safe_name}.vrd")));
            }
            (active.dir.clone(), active.vrd.clone().unwrap())
        }; // lock released before file I/O
        let project = vinrouge::projects::load_project(&dir)?;
        vinrouge::projects::vrd::export_project_vrd(&dir, &project, &vrd_path)
    }
}

/// Caches the DuckDB datasource so SQLite is only read once per import session.
/// Tracks the script currently open in the studio editor so `./vu current` always reflects live state.
pub struct CurrentScriptState(pub Mutex<Option<vinrouge::projects::DslScript>>);

impl Default for CurrentScriptState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

pub struct DslCacheState(pub Arc<Mutex<DslCache>>);

pub struct DslCache {
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
