use crate::helpers::{build_datasource, run_dsl_script_blocking, run_dsl_script_text_blocking};
use crate::state::{DslCacheState, ProjectsState};
use std::sync::Arc;
use tauri::State;

/// Drop the cached DuckDB datasource so the next script run rebuilds from SQLite.
/// Useful when the user knows data has changed outside the normal import flow.
#[tauri::command]
pub fn invalidate_dsl_cache(cache: State<'_, DslCacheState>) {
    cache.0.lock().unwrap().datasource = None;
}

#[tauri::command]
pub fn save_dsl_script(
    control_id: String,
    control_ref: String,
    label: String,
    script_text: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::DslScript, String> {
    let project_dir = state.dir()?;
    let script = vinrouge::projects::save_dsl_script(&project_dir, &control_id, &control_ref, &label, &script_text)?;
    state.sync_vrd()?;
    Ok(script)
}

#[tauri::command]
pub fn list_dsl_scripts(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::DslScript>, String> {
    let project_dir = state.dir()?;
    vinrouge::projects::list_dsl_scripts(&project_dir)
}

#[tauri::command]
pub fn clear_dsl_scripts(state: State<ProjectsState>) -> Result<(), String> {
    let project_dir = state.dir()?;
    vinrouge::projects::clear_dsl_scripts(&project_dir)?;
    state.sync_vrd()
}

/// Execute a saved DSL script against all session rows and save the results.
/// The DuckDB datasource is cached in [`DslCacheState`] — the slow SQLite →
/// JSON → DuckDB build only runs on the first call after an import.
#[tauri::command]
pub async fn run_dsl_script(
    script_id: String,
    state: State<'_, ProjectsState>,
    cache: State<'_, DslCacheState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.dir()?;
    let cache_arc = cache.0.clone();

    tokio::task::spawn_blocking(move || {
        // Get or build the datasource — the expensive SQLite read only happens
        // on a cache miss (first run after an import or project change).
        let datasource = {
            let mut guard = cache_arc.lock().unwrap();
            let same_project = guard.project_dir.as_ref() == Some(&project_dir);
            if same_project {
                if let Some(ds) = guard.datasource.clone() {
                    ds
                } else {
                    let ds = Arc::new(build_datasource(&project_dir)?);
                    guard.datasource = Some(ds.clone());
                    ds
                }
            } else {
                let ds = Arc::new(build_datasource(&project_dir)?);
                guard.project_dir = Some(project_dir.clone());
                guard.datasource = Some(ds.clone());
                ds
            }
        };

        run_dsl_script_blocking(script_id, project_dir, datasource)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_test_results(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::TestResult>, String> {
    let project_dir = state.dir()?;
    vinrouge::projects::list_test_results(&project_dir)
}

#[tauri::command]
pub fn delete_dsl_script(
    script_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.dir()?;
    vinrouge::projects::delete_dsl_script(&project_dir, &script_id)?;
    state.sync_vrd()
}

#[tauri::command]
pub fn update_dsl_script(
    script_id: String,
    script_text: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.dir()?;
    vinrouge::projects::update_dsl_script(&project_dir, &script_id, &script_text)?;
    state.sync_vrd()
}

#[tauri::command]
pub fn rename_dsl_script(
    script_id: String,
    label: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.dir()?;
    vinrouge::projects::rename_dsl_script(&project_dir, &script_id, &label)?;
    state.sync_vrd()
}

#[tauri::command]
pub async fn run_all_dsl_scripts(
    state: State<'_, ProjectsState>,
    cache: State<'_, DslCacheState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.dir()?;
    let cache_arc = cache.0.clone();

    tokio::task::spawn_blocking(move || {
        let datasource = {
            let mut guard = cache_arc.lock().unwrap();
            let same_project = guard.project_dir.as_ref() == Some(&project_dir);
            if same_project {
                if let Some(ds) = guard.datasource.clone() {
                    ds
                } else {
                    let ds = Arc::new(build_datasource(&project_dir)?);
                    guard.datasource = Some(ds.clone());
                    ds
                }
            } else {
                let ds = Arc::new(build_datasource(&project_dir)?);
                guard.project_dir = Some(project_dir.clone());
                guard.datasource = Some(ds.clone());
                ds
            }
        };

        let scripts = vinrouge::projects::list_dsl_scripts(&project_dir)?;
        let mut all_results: Vec<serde_json::Value> = Vec::new();

        for script in scripts {
            let json_results = run_dsl_script_text_blocking(
                &script.script_text,
                &project_dir,
                datasource.clone(),
                Some(&script.id),
            )?;

            let passed = json_results
                .iter()
                .filter(|r| r["kind"] == "assert" && r["passed"] == true)
                .count() as i64;
            let failed = json_results
                .iter()
                .filter(|r| r["kind"] == "assert" && r["passed"] == false)
                .count() as i64;
            let errors = json_results
                .iter()
                .filter(|r| r["kind"] == "error")
                .count() as i64;

            all_results.push(serde_json::json!({
                "kind": "section",
                "title": script.label,
                "passed": passed,
                "failed": failed,
                "errors": errors,
                "results": json_results,
            }));
        }

        Ok(all_results)
    })
    .await
    .map_err(|e| e.to_string())?
}
