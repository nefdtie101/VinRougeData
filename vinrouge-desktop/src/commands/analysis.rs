use crate::helpers::run_analysis;
use crate::state::{AnalysisOutput, ProjectsState};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

/// Opens the OS file picker on the main thread (via callback), then runs
/// analysis on a dedicated thread. Using async + oneshot avoids the macOS
/// spinning-beachball caused by blocking_pick_file() deadlocking the main thread.
#[tauri::command]
pub async fn pick_and_analyze(app: AppHandle) -> Result<Option<AnalysisOutput>, String> {
    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .pick_file(move |fp| {
            let _ = dialog_tx.send(fp);
        });

    let Some(fp) = dialog_rx.await.map_err(|e| e.to_string())? else {
        return Ok(None); // user cancelled
    };

    let path = match fp {
        FilePath::Path(p) => p.to_string_lossy().to_string(),
        FilePath::Url(u) => u.to_string(),
    };

    // DataSource futures are !Send — run on a dedicated current_thread runtime.
    let (analysis_tx, analysis_rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(run_analysis(path)));
        let _ = analysis_tx.send(result);
    });

    analysis_rx.await.map_err(|e| e.to_string())?.map(Some)
}

/// Pick a file, analyse it, copy it into the active project, and persist the result.
#[tauri::command]
pub async fn pick_analyze_and_save(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<Option<AnalysisOutput>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };

    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .pick_file(move |fp| {
            let _ = dialog_tx.send(fp);
        });

    let Some(fp) = dialog_rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };

    let src_str = match &fp {
        FilePath::Path(p) => p.to_string_lossy().to_string(),
        FilePath::Url(u) => u.to_string(),
    };
    let src_path = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    // Copy file into project
    let pf = vinrouge::projects::add_file_to_project(&project_dir, &src_path)
        .map_err(|e| format!("Failed to add file: {e}"))?;

    // Run !Send analysis on a dedicated current_thread runtime
    let (analysis_tx, analysis_rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(run_analysis(src_str)));
        let _ = analysis_tx.send(result);
    });

    let analysis = analysis_rx.await.map_err(|e| e.to_string())??;

    // Persist result JSON
    let json = serde_json::to_string(&analysis).map_err(|e| e.to_string())?;
    vinrouge::projects::save_analysis(&project_dir, &pf.id, &json)?;

    Ok(Some(analysis))
}
