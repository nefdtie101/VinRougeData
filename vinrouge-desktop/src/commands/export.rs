use crate::state::ProjectsState;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

/// Export the current audit plan to PDF. Opens a save-file dialog.
#[tauri::command]
pub async fn export_audit_plan_pdf(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.dir()?;
    let processes = vinrouge::projects::list_audit_plan(&project_dir)?;
    let details = vinrouge::projects::load_project_details(&project_dir)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<FilePath>>();
    app.dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name("audit-plan.pdf")
        .save_file(move |fp| {
            let _ = tx.send(fp);
        });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let path = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    vinrouge::export::audit_plan::generate_pdf(&processes, details.as_ref(), &path)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Export the current audit plan to Word (.docx). Opens a save-file dialog.
#[tauri::command]
pub async fn export_audit_plan_docx(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.dir()?;
    let processes = vinrouge::projects::list_audit_plan(&project_dir)?;
    let details = vinrouge::projects::load_project_details(&project_dir)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<FilePath>>();
    app.dialog()
        .file()
        .add_filter("Word document", &["docx"])
        .set_file_name("audit-plan.docx")
        .save_file(move |fp| {
            let _ = tx.send(fp);
        });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let path = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    vinrouge::export::audit_plan::generate_docx(&processes, details.as_ref(), &path)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Export the PBC data request list to PDF. Opens a save-file dialog.
#[tauri::command]
pub async fn export_pbc_pdf(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.dir()?;
    let groups = vinrouge::projects::list_pbc_groups(&project_dir)?;
    let details = vinrouge::projects::load_project_details(&project_dir)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<FilePath>>();
    app.dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name("pbc-list.pdf")
        .save_file(move |fp| {
            let _ = tx.send(fp);
        });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let path = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    vinrouge::export::pbc_list::generate_pdf(&groups, details.as_ref(), &path)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Export the PBC data request list to Word (.docx). Opens a save-file dialog.
#[tauri::command]
pub async fn export_pbc_docx(
    app: AppHandle,
    state: State<'_, ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.dir()?;
    let groups = vinrouge::projects::list_pbc_groups(&project_dir)?;
    let details = vinrouge::projects::load_project_details(&project_dir)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<FilePath>>();
    app.dialog()
        .file()
        .add_filter("Word document", &["docx"])
        .set_file_name("pbc-list.docx")
        .save_file(move |fp| {
            let _ = tx.send(fp);
        });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let path = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    vinrouge::export::pbc_list::generate_docx(&groups, details.as_ref(), &path)
        .map_err(|e| e.to_string())?;
    Ok(true)
}
