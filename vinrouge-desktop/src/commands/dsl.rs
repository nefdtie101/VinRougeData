use crate::helpers::run_dsl_script_blocking;
use crate::state::ProjectsState;
use tauri::State;

#[tauri::command]
pub fn save_dsl_script(
    control_id: String,
    control_ref: String,
    label: String,
    script_text: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::DslScript, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::save_dsl_script(&project_dir, &control_id, &control_ref, &label, &script_text)
}

#[tauri::command]
pub fn list_dsl_scripts(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::DslScript>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::list_dsl_scripts(&project_dir)
}

#[tauri::command]
pub fn clear_dsl_scripts(state: State<ProjectsState>) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::clear_dsl_scripts(&project_dir)
}

/// Execute a saved DSL script against all session rows and save the results.
#[tauri::command]
pub async fn run_dsl_script(
    script_id: String,
    state: State<'_, ProjectsState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || run_dsl_script_blocking(script_id, project_dir))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_test_results(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::TestResult>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::list_test_results(&project_dir)
}

#[tauri::command]
pub fn delete_dsl_script(
    script_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::delete_dsl_script(&project_dir, &script_id)
}

#[tauri::command]
pub fn update_dsl_script(
    script_id: String,
    script_text: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::update_dsl_script(&project_dir, &script_id, &script_text)
}

#[tauri::command]
pub fn rename_dsl_script(
    script_id: String,
    label: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::rename_dsl_script(&project_dir, &script_id, &label)
}
