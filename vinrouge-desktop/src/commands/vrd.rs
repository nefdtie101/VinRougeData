use crate::state::ProjectsState;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

// ── Save (re-pack into linked .vrd) ──────────────────────────────────────────

/// Re-packs the active project into its `.vrd` file.
/// If no `.vrd` is linked, one is created at `~/VinRouge/vrd/<name>.vrd`
/// and linked automatically so future saves write to the same file.
#[tauri::command]
pub fn save_project_vrd(state: State<'_, ProjectsState>) -> Result<String, String> {
    state.sync_vrd()?;
    Ok(state
        .get_vrd()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default())
}

// ── Export (Save As, with file dialog) ───────────────────────────────────────

#[tauri::command]
pub async fn export_project_vrd(
    app: tauri::AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<String, String> {
    let project_dir = state.dir()?;
    let project = vinrouge::projects::load_project(&project_dir)?;
    let default_name = format!("{}.vrd", project.name);

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("VinRouge Project", &["vrd"])
        .save_file(move |fp| {
            let _ = tx.send(fp);
        });

    let picked = rx.await.map_err(|e| e.to_string())?;
    if let Some(fp) = picked {
        let dest = match fp {
            tauri_plugin_dialog::FilePath::Path(p) => p,
            tauri_plugin_dialog::FilePath::Url(u) => return Err(format!("Invalid save path: {u}")),
        };
        vinrouge::projects::vrd::export_project_vrd(&project_dir, &project, &dest)?;
        let dest_str = dest.to_string_lossy().to_string();
        state.set_vrd(dest);
        Ok(dest_str)
    } else {
        Ok(String::new()) // user cancelled
    }
}

// ── Pick + open ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn pick_and_open_vrd(
    app: tauri::AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<Option<vinrouge::projects::Project>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("VinRouge Project", &["vrd"])
        .pick_file(move |fp| {
            let _ = tx.send(fp);
        });

    let picked = rx.await.map_err(|e| e.to_string())?;
    let vrd_path = match picked {
        None => return Ok(None),
        Some(tauri_plugin_dialog::FilePath::Path(p)) => p,
        Some(tauri_plugin_dialog::FilePath::Url(u)) => {
            return Err(format!("Invalid file path: {u}"))
        }
    };

    let home = vinrouge::projects::vinrouge_home()?;
    let parent = home.join("projects");
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    let extracted = vinrouge::projects::vrd::import_project_vrd(&vrd_path, &parent)?;
    let project_dir = std::path::PathBuf::from(&extracted.path);
    let mut project = vinrouge::projects::load_project(&project_dir)?;
    project.vrd_path = Some(vrd_path.to_string_lossy().to_string());

    state.activate(project_dir, Some(vrd_path));
    Ok(Some(project))
}

// ── Import (adds to list without opening) ────────────────────────────────────

#[tauri::command]
pub async fn import_project_vrd(
    app: tauri::AppHandle,
) -> Result<Option<vinrouge::projects::Project>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("VinRouge Project", &["vrd"])
        .pick_file(move |fp| {
            let _ = tx.send(fp);
        });

    let picked = rx.await.map_err(|e| e.to_string())?;
    if let Some(fp) = picked {
        let src = match fp {
            tauri_plugin_dialog::FilePath::Path(p) => p,
            tauri_plugin_dialog::FilePath::Url(u) => return Err(format!("Invalid file path: {u}")),
        };
        let home = vinrouge::projects::vinrouge_home()?;
        let parent = home.join("projects");
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let project = vinrouge::projects::vrd::import_project_vrd(&src, &parent)?;
        Ok(Some(project))
    } else {
        Ok(None)
    }
}
