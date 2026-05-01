use crate::state::ProjectsState;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use vinrouge::sources::DataSource;

#[tauri::command]
pub async fn pick_and_add_file(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<Option<vinrouge::projects::ProjectFile>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .add_filter("SOP / Documents", &["pdf", "txt"])
        .add_filter("All supported", &["csv", "xlsx", "xls", "pdf", "txt"])
        .pick_file(move |fp| {
            let _ = tx.send(fp);
        });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };

    let src = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    vinrouge::projects::add_file_to_project(&project_dir, &src).map(Some)
}

#[tauri::command]
pub fn list_project_files(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::ProjectFile>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    vinrouge::projects::list_project_files(&project_dir)
}

#[tauri::command]
pub fn read_project_file(
    file_id: String,
    state: State<ProjectsState>,
) -> Result<String, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::read_project_file_text(&project_dir, &file_id)
}

#[tauri::command]
pub fn add_data_file(
    name: String,
    bytes: Vec<u8>,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::ProjectFile, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::add_file_bytes_to_project(&project_dir, &name, &bytes)
}

#[tauri::command]
pub fn delete_project_file(
    file_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::delete_project_file(&project_dir, &file_id)
}

/// Return the column headers for a CSV or Excel project file.
#[tauri::command]
pub async fn get_data_file_headers(
    file_id: String,
    state: State<'_, ProjectsState>,
) -> Result<Vec<String>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let path = vinrouge::projects::get_file_path(&project_dir, &file_id)?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| {
                rt.block_on(async {
                    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
                    let tables: Vec<vinrouge::schema::Table> = if ext == "csv" {
                        vinrouge::sources::CsvSource::from_bytes(bytes, name)
                            .extract_schema()
                            .await
                            .map_err(|e| e.to_string())?
                    } else {
                        vinrouge::sources::ExcelSource::from_bytes(bytes, name)
                            .extract_schema()
                            .await
                            .map_err(|e| e.to_string())?
                    };
                    let mut seen = std::collections::HashSet::new();
                    let headers = tables
                        .into_iter()
                        .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                        .filter(|h| {
                            let t = h.trim();
                            !t.is_empty() && seen.insert(t.to_string())
                        })
                        .collect::<Vec<_>>();
                    Ok(headers)
                })
            });
        let _ = tx.send(result);
    });
    rx.await.map_err(|e| e.to_string())?
}
