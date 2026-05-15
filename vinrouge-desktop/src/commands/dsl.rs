use crate::helpers::{build_datasource, run_dsl_script_blocking, run_dsl_script_text_blocking};
use crate::state::{DslCacheState, ProjectsState};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

fn emit_status(
    app: &AppHandle,
    phase: &str,
    label: Option<&str>,
    n: Option<usize>,
    total: Option<usize>,
) {
    let mut v = serde_json::json!({"phase": phase});
    if let Some(l) = label {
        v["label"] = serde_json::Value::String(l.to_string());
    }
    if let Some(n) = n {
        v["n"] = serde_json::Value::Number(n.into());
    }
    if let Some(t) = total {
        v["total"] = serde_json::Value::Number(t.into());
    }
    let _ = app.emit("dsl-status", v);
}

/// Drop the cached DuckDB datasource so the next script run rebuilds from SQLite.
/// Useful when the user knows data has changed outside the normal import flow.
#[tauri::command]
pub fn invalidate_dsl_cache(cache: State<'_, DslCacheState>) {
    cache.0.lock().unwrap().datasource = None;
}

#[tauri::command]
pub async fn save_dsl_script(
    control_id: String,
    control_ref: String,
    label: String,
    script_text: String,
    state: State<'_, ProjectsState>,
) -> Result<vinrouge::projects::DslScript, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::save_dsl_script(
            &project_dir,
            &control_id,
            &control_ref,
            &label,
            &script_text,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_dsl_scripts(
    state: State<'_, ProjectsState>,
) -> Result<Vec<vinrouge::projects::DslScript>, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::list_dsl_scripts(&project_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_dsl_scripts(state: State<'_, ProjectsState>) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::clear_dsl_scripts(&project_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Execute a saved DSL script against all session rows and save the results.
/// The DuckDB datasource is cached in [`DslCacheState`] — the slow SQLite →
/// JSON → DuckDB build only runs on the first call after an import.
#[tauri::command]
pub async fn run_dsl_script(
    app: AppHandle,
    script_id: String,
    state: State<'_, ProjectsState>,
    cache: State<'_, DslCacheState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.dir()?;
    let cache_arc = cache.0.clone();

    tokio::task::spawn_blocking(move || {
        let datasource = {
            let mut guard = cache_arc.lock().unwrap();
            let cache_hit =
                guard.project_dir.as_ref() == Some(&project_dir) && guard.datasource.is_some();
            if cache_hit {
                guard.datasource.clone().unwrap()
            } else {
                emit_status(&app, "loading", None, None, None);
                let ds = Arc::new(build_datasource(&project_dir)?);
                guard.project_dir = Some(project_dir.clone());
                guard.datasource = Some(ds.clone());
                ds
            }
        };

        emit_status(&app, "running", None, None, None);
        run_dsl_script_blocking(script_id, project_dir, datasource)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_test_results(
    state: State<'_, ProjectsState>,
) -> Result<Vec<vinrouge::projects::TestResult>, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::list_test_results(&project_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_dsl_script(
    script_id: String,
    state: State<'_, ProjectsState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::delete_dsl_script(&project_dir, &script_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_dsl_script(
    script_id: String,
    script_text: String,
    state: State<'_, ProjectsState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::update_dsl_script(&project_dir, &script_id, &script_text)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn rename_dsl_script(
    script_id: String,
    label: String,
    state: State<'_, ProjectsState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_dir = state.dir()?;
        vinrouge::projects::rename_dsl_script(&project_dir, &script_id, &label)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn run_all_dsl_scripts(
    app: AppHandle,
    state: State<'_, ProjectsState>,
    cache: State<'_, DslCacheState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.dir()?;
    let cache_arc = cache.0.clone();

    tokio::task::spawn_blocking(move || {
        let datasource = {
            let mut guard = cache_arc.lock().unwrap();
            let cache_hit =
                guard.project_dir.as_ref() == Some(&project_dir) && guard.datasource.is_some();
            if cache_hit {
                guard.datasource.clone().unwrap()
            } else {
                emit_status(&app, "loading", None, None, None);
                let ds = Arc::new(build_datasource(&project_dir)?);
                guard.project_dir = Some(project_dir.clone());
                guard.datasource = Some(ds.clone());
                ds
            }
        };

        let scripts = vinrouge::projects::list_dsl_scripts(&project_dir)?;
        let total = scripts.len();
        let mut all_results: Vec<serde_json::Value> = Vec::new();

        for (i, script) in scripts.into_iter().enumerate() {
            emit_status(
                &app,
                "running",
                Some(&script.label),
                Some(i + 1),
                Some(total),
            );
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
            let errors = json_results.iter().filter(|r| r["kind"] == "error").count() as i64;

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
