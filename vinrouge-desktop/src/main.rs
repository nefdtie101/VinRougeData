// Prevent a console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};
use vinrouge::analysis::{RelationshipDetector, Workflow, WorkflowDetector};
use vinrouge::ollama;
use vinrouge::projects;
use vinrouge::schema::{Relationship, Table};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};

// ── Ollama process state ──────────────────────────────────────────────────────

struct OllamaState(Mutex<Option<std::process::Child>>);

// ── Projects state (currently-open project directory) ─────────────────────────

struct ProjectsState(Mutex<Option<PathBuf>>);

// ── Output type sent back to the Leptos frontend via Tauri IPC ───────────────

#[derive(Serialize)]
struct AnalysisOutput {
    tables: Vec<Table>,
    relationships: Vec<Relationship>,
    workflows: Vec<Workflow>,
}

// ── Core analysis (runs in its own current-thread Tokio runtime) ──────────────
//
// DataSource::extract_schema() uses `async_trait(?Send)` for WASM compat, so
// its Future is !Send.  We must run it on a current_thread runtime; spinning
// one up on a fresh OS thread is the cleanest way to do that.

async fn run_analysis(path: String) -> Result<AnalysisOutput, String> {
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let tables: Vec<Table> = if ext == "csv" {
        CsvSource::new(path.clone())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else if ext == "xlsx" || ext == "xls" {
        ExcelSource::new(path.clone())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else {
        return Err(format!("Unsupported file type: .{ext}"));
    };

    let relationships = RelationshipDetector::new(tables.clone()).detect_relationships();
    let workflows =
        WorkflowDetector::new(tables.clone(), relationships.clone()).detect_workflows();

    Ok(AnalysisOutput { tables, relationships, workflows })
}

// ── Tauri command: open native file dialog, then run analysis ────────────────

/// Opens the OS file picker on the main thread (via callback), then runs
/// analysis on a dedicated thread. Using async + oneshot avoids the macOS
/// spinning-beachball caused by blocking_pick_file() deadlocking the main thread.
#[tauri::command]
async fn pick_and_analyze(app: tauri::AppHandle) -> Result<Option<AnalysisOutput>, String> {
    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .pick_file(move |fp| { let _ = dialog_tx.send(fp); });

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

// ── Ollama commands ───────────────────────────────────────────────────────────

/// Start the bundled Ollama server. Safe to call multiple times — does nothing
/// if it is already running. Returns the resolved binary path on success.
#[tauri::command]
fn start_ollama(state: tauri::State<OllamaState>) -> Result<String, String> {
    let mut guard = state.0.lock().unwrap();

    // Already running?
    if let Some(child) = guard.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return Ok("already running".to_string());
        }
    }

    let binary = ollama::find_binary().map_err(|e| e.to_string())?;
    let binary_str = binary.to_string_lossy().to_string();

    let mut cmd = std::process::Command::new(&binary);
    cmd.arg("serve");

    // Read user override from shared settings file, fall back to DEFAULT_MODELS_DIR
    let saved_dir: Option<String> = (|| {
        let home = std::env::var("HOME").ok()?;
        let path = std::path::PathBuf::from(home)
            .join(".config").join("vinrouge").join("tui.toml");
        let content = std::fs::read_to_string(path).ok()?;
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("ollama_models_dir = ") {
                let dir = val.trim().trim_matches('"').to_string();
                if !dir.is_empty() { return Some(dir); }
            }
        }
        None
    })();

    if let Some(dir) = ollama::resolve_models_dir(saved_dir.as_deref()) {
        cmd.env("OLLAMA_MODELS", dir);
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to start Ollama: {e}"))?;
    *guard = Some(child);
    Ok(binary_str)
}

/// Stop the Ollama server that was started by this app.
#[tauri::command]
fn stop_ollama(state: tauri::State<OllamaState>) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        child.kill().map_err(|e| e.to_string())?;
    }
    *guard = None;
    Ok(())
}

/// Returns `true` if the Ollama process we spawned is still alive.
#[tauri::command]
fn ollama_running(state: tauri::State<OllamaState>) -> bool {
    let mut guard = state.0.lock().unwrap();
    match guard.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

// ── Project commands ──────────────────────────────────────────────────────────

#[tauri::command]
fn create_project(
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
) -> Result<projects::Project, String> {
    let home = projects::vinrouge_home()?;
    let parent = match save_dir {
        Some(dir) => PathBuf::from(dir),
        None => home.join("projects"),
    };
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let project = projects::create_project(&name, &parent)?;
    let details = projects::ProjectDetails {
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
    projects::save_project_details(&std::path::PathBuf::from(&project.path), &details)?;
    Ok(project)
}

#[tauri::command]
async fn pick_project_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let default_dir = projects::vinrouge_home()
        .map(|h| h.join("projects"))
        .ok();

    let (tx, rx) = tokio::sync::oneshot::channel();

    let mut dialog = app.dialog().file();
    if let Some(dir) = default_dir {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder(move |fp| { let _ = tx.send(fp); });

    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked.map(|fp| match fp {
        FilePath::Path(p) => p.to_string_lossy().to_string(),
        FilePath::Url(u) => u.to_string(),
    }))
}

#[tauri::command]
fn list_projects() -> Result<Vec<projects::Project>, String> {
    let home = projects::vinrouge_home()?;
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    projects::list_projects(&home)
}

#[tauri::command]
fn delete_project(path: String) -> Result<(), String> {
    projects::delete_project(&PathBuf::from(path))
}

#[tauri::command]
fn open_project(
    path: String,
    state: tauri::State<ProjectsState>,
) -> Result<projects::Project, String> {
    let project_path = PathBuf::from(&path);
    let project = projects::load_project(&project_path)?;
    *state.0.lock().unwrap() = Some(project_path);
    Ok(project)
}

#[tauri::command]
fn load_project_details(
    state: tauri::State<ProjectsState>,
) -> Result<Option<projects::ProjectDetails>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    projects::load_project_details(&project_dir)
}

#[tauri::command]
fn get_active_project(state: tauri::State<ProjectsState>) -> Result<Option<String>, String> {
    let guard = state.0.lock().unwrap();
    Ok(guard.as_ref().map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
async fn pick_and_add_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectsState>,
) -> Result<Option<projects::ProjectFile>, String> {
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
        .pick_file(move |fp| { let _ = tx.send(fp); });

    let Some(fp) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };

    let src = match fp {
        FilePath::Path(p) => p,
        FilePath::Url(u) => PathBuf::from(u.to_string()),
    };

    projects::add_file_to_project(&project_dir, &src).map(Some)
}

#[tauri::command]
fn list_project_files(
    state: tauri::State<ProjectsState>,
) -> Result<Vec<projects::ProjectFile>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    projects::list_project_files(&project_dir)
}

#[tauri::command]
fn save_ai_message(
    role: String,
    content: String,
    state: tauri::State<ProjectsState>,
) -> Result<projects::AiMessage, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    projects::save_ai_message(&project_dir, &role, &content)
}

#[tauri::command]
fn list_ai_messages(
    state: tauri::State<ProjectsState>,
) -> Result<Vec<projects::AiMessage>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };
    projects::list_ai_messages(&project_dir)
}

#[tauri::command]
async fn pick_analyze_and_save(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectsState>,
) -> Result<Option<AnalysisOutput>, String> {
    let project_dir = {
        let guard = state.0.lock().unwrap();
        guard.clone().ok_or("No active project")?
    };

    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .pick_file(move |fp| { let _ = dialog_tx.send(fp); });

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
    let pf = projects::add_file_to_project(&project_dir, &src_path)
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
    projects::save_analysis(&project_dir, &pf.id, &json)?;

    Ok(Some(analysis))
}

// ── SOP / Audit-plan commands ─────────────────────────────────────────────────

/// Read the text content of a project file so the frontend can send it to Ollama.
#[tauri::command]
fn read_project_file(
    file_id: String,
    state: tauri::State<ProjectsState>,
) -> Result<String, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::read_project_file_text(&project_dir, &file_id)
}

/// Persist an AI-generated audit plan for a SOP file.
/// `processes_json` is the raw JSON string the frontend received from Ollama,
/// shaped as `{ "processes": [{process_name, description, controls:[...]}] }`.
#[tauri::command]
fn save_audit_plan(
    sop_file_id: String,
    processes_json: String,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;

    #[derive(serde::Deserialize)]
    struct ControlDto {
        control_ref:         String,
        control_objective:   String,
        control_description: String,
        test_procedure:      String,
        risk_level:          String,
    }
    #[derive(serde::Deserialize)]
    struct ProcessDto {
        process_name: String,
        description:  String,
        controls:     Vec<ControlDto>,
    }
    #[derive(serde::Deserialize)]
    struct PlanDto { processes: Vec<ProcessDto> }

    let plan: PlanDto = serde_json::from_str(&processes_json)
        .map_err(|e| format!("Invalid plan JSON: {e}"))?;

    let batch: Vec<(String, String, Vec<(String, String, String, String, String)>)> = plan
        .processes
        .into_iter()
        .map(|p| {
            let controls = p.controls.into_iter().map(|c| {
                (c.control_ref, c.control_objective, c.control_description, c.test_procedure, c.risk_level)
            }).collect();
            (p.process_name, p.description, controls)
        })
        .collect();

    projects::replace_audit_plan(&project_dir, &sop_file_id, &batch)
}

/// Return the current audit plan (all processes + controls) for the active project.
#[tauri::command]
fn list_audit_plan(
    state: tauri::State<ProjectsState>,
) -> Result<Vec<projects::AuditProcessWithControls>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::list_audit_plan(&project_dir)
}

#[tauri::command]
fn add_control(
    process_id: String,
    control_ref: String,
    control_objective: String,
    control_description: String,
    test_procedure: String,
    risk_level: String,
    state: tauri::State<ProjectsState>,
) -> Result<projects::Control, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::add_control(
        &project_dir,
        &process_id,
        &control_ref,
        &control_objective,
        &control_description,
        &test_procedure,
        &risk_level,
    )
}

#[tauri::command]
fn delete_control(
    control_id: String,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::delete_control(&project_dir, &control_id)
}

/// Patch a single field on a control row.
#[tauri::command]
fn update_control_field(
    control_id: String,
    field: String,
    value: String,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::update_control_field(&project_dir, &control_id, &field, &value)
}

/// Patch a single field on a process row.
#[tauri::command]
fn update_process_field(
    process_id: String,
    field: String,
    value: String,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::update_process_field(&project_dir, &process_id, &field, &value)
}

#[tauri::command]
fn list_pbc_groups(
    state: tauri::State<ProjectsState>,
) -> Result<Vec<projects::PbcGroup>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::list_pbc_groups(&project_dir)
}

#[tauri::command]
fn save_pbc_item(
    control_id: String,
    control_ref: String,
    name: String,
    item_type: String,
    table_name: Option<String>,
    fields: Vec<String>,
    purpose: String,
    scope_format: String,
    state: tauri::State<ProjectsState>,
) -> Result<projects::PbcItem, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::save_pbc_item(
        &project_dir, &control_id, &control_ref, &name, &item_type,
        table_name.as_deref(), &fields, &purpose, &scope_format,
    )
}

#[tauri::command]
fn delete_pbc_item(
    item_id: String,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::delete_pbc_item(&project_dir, &item_id)
}

#[tauri::command]
fn clear_pbc_items(
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::clear_pbc_items(&project_dir)
}

#[tauri::command]
fn update_pbc_item_fields(
    item_id: String,
    fields: Vec<String>,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::update_pbc_item_fields(&project_dir, &item_id, &fields)
}

#[tauri::command]
fn toggle_pbc_item_approved(
    item_id: String,
    state: tauri::State<ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::toggle_pbc_item_approved(&project_dir, &item_id)
}

#[tauri::command]
fn get_pbc_list_approved(
    state: tauri::State<ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::get_pbc_list_approved(&project_dir)
}

#[tauri::command]
fn set_pbc_list_approved(
    approved: bool,
    state: tauri::State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    projects::set_pbc_list_approved(&project_dir, approved)
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(OllamaState(Mutex::new(None)))
        .manage(ProjectsState(Mutex::new(None)))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_and_analyze,
            start_ollama,
            stop_ollama,
            ollama_running,
            create_project,
            pick_project_folder,
            list_projects,
            open_project,
            delete_project,
            load_project_details,
            get_active_project,
            pick_and_add_file,
            list_project_files,
            save_ai_message,
            list_ai_messages,
            pick_analyze_and_save,
            read_project_file,
            save_audit_plan,
            list_audit_plan,
            add_control,
            delete_control,
            update_control_field,
            update_process_field,
            list_pbc_groups,
            save_pbc_item,
            delete_pbc_item,
            clear_pbc_items,
            update_pbc_item_fields,
            toggle_pbc_item_approved,
            get_pbc_list_approved,
            set_pbc_list_approved,
        ])
        .setup(|app| {
            // Auto-start Ollama when the desktop app launches
            let state = app.state::<OllamaState>();
            match ollama::find_binary() {
                Err(e) => eprintln!("[ollama] binary not found: {e}"),
                Ok(binary) => {
                    eprintln!("[ollama] found binary: {}", binary.display());
                    let mut cmd = std::process::Command::new(binary);
                    cmd.arg("serve");
                    if let Some(dir) = ollama::resolve_models_dir(None) {
                        eprintln!("[ollama] OLLAMA_MODELS={dir}");
                        cmd.env("OLLAMA_MODELS", dir);
                    }
                    match cmd.spawn() {
                        Ok(child) => {
                            eprintln!("[ollama] started (pid {})", child.id());
                            *state.0.lock().unwrap() = Some(child);
                        }
                        Err(e) => eprintln!("[ollama] failed to spawn: {e}"),
                    }
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building VinRouge")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<OllamaState>();
                let mut guard = state.0.lock().unwrap();
                if let Some(child) = guard.as_mut() {
                    let _ = child.kill();
                    eprintln!("[ollama] killed on exit");
                }
            }
        });
}
