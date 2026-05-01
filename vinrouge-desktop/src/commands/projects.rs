use crate::state::ProjectsState;
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn create_project(
    name: String,
    save_dir: Option<String>,
    client: String,
    engagement_ref: String,
    period_start: String,
    period_end: String,
    report_due: String,
    audit_type: String,
    notes: String,
    standards: Vec<String>,
    scope: String,
    materiality: String,
    risk_framework: String,
) -> Result<vinrouge::projects::Project, String> {
    let home = vinrouge::projects::vinrouge_home()?;
    let parent = match save_dir {
        Some(dir) => PathBuf::from(dir),
        None => home.join("projects"),
    };
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let project = vinrouge::projects::create_project(&name, &parent)?;
    let details = vinrouge::projects::ProjectDetails {
        client,
        engagement_ref,
        period_start,
        period_end,
        report_due,
        audit_type,
        notes,
        standards,
        scope,
        materiality,
        risk_framework,
    };
    vinrouge::projects::save_project_details(
        &std::path::PathBuf::from(&project.path),
        &details,
    )?;
    Ok(project)
}

#[tauri::command]
pub async fn pick_project_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let default_dir = vinrouge::projects::vinrouge_home()
        .map(|h| h.join("projects"))
        .ok();

    let (tx, rx) = tokio::sync::oneshot::channel();

    let mut dialog = app.dialog().file();
    if let Some(dir) = default_dir {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder(move |fp| {
        let _ = tx.send(fp);
    });

    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked.map(|fp| match fp {
        tauri_plugin_dialog::FilePath::Path(p) => p.to_string_lossy().to_string(),
        tauri_plugin_dialog::FilePath::Url(u) => u.to_string(),
    }))
}

#[tauri::command]
pub fn list_projects() -> Result<Vec<vinrouge::projects::Project>, String> {
    let home = vinrouge::projects::vinrouge_home()?;
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    vinrouge::projects::list_projects(&home)
}

#[tauri::command]
pub fn delete_project(path: String) -> Result<(), String> {
    vinrouge::projects::delete_project(&PathBuf::from(path))
}

#[tauri::command]
pub fn open_project(
    path: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::Project, String> {
    let project_path = PathBuf::from(&path);
    let project = vinrouge::projects::load_project(&project_path)?;
    *state.0.lock().unwrap() = Some(project_path);
    Ok(project)
}

#[tauri::command]
pub fn load_project_details(
    state: State<ProjectsState>,
) -> Result<Option<vinrouge::projects::ProjectDetails>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    vinrouge::projects::load_project_details(&project_dir)
}

#[tauri::command]
pub fn get_active_project(state: State<ProjectsState>) -> Result<Option<String>, String> {
    let guard = state.0.lock().unwrap();
    Ok(guard.as_ref().map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn save_ai_message(
    role: String,
    content: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::AiMessage, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    vinrouge::projects::save_ai_message(&project_dir, &role, &content)
}

#[tauri::command]
pub fn list_ai_messages(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::AiMessage>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    vinrouge::projects::list_ai_messages(&project_dir)
}
