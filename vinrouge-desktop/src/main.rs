// Prevent a console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};
use vinrouge::analysis::{RelationshipDetector, Workflow, WorkflowDetector};
use vinrouge::ollama;
use vinrouge::schema::{Relationship, Table};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};

// ── Ollama process state ──────────────────────────────────────────────────────

struct OllamaState(Mutex<Option<std::process::Child>>);

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

/// Synchronous command — Tauri will call this on a thread-pool thread, so
/// blocking operations (dialog, I/O) are fine here.
#[tauri::command]
fn pick_and_analyze(app: tauri::AppHandle) -> Result<Option<AnalysisOutput>, String> {
    // Show OS-native file picker (blocks until the user picks or cancels).
    let file_path: Option<FilePath> = app
        .dialog()
        .file()
        .add_filter("Data Files", &["csv", "xlsx", "xls"])
        .blocking_pick_file();

    let Some(fp) = file_path else {
        return Ok(None); // user cancelled
    };

    let path = match fp {
        FilePath::Path(p) => p.to_string_lossy().to_string(),
        FilePath::Url(u) => u.to_string(),
    };

    // The DataSource futures are !Send (async_trait ?Send for WASM compat).
    // Spin up a dedicated OS thread with a fresh current_thread Tokio runtime
    // so we can .await them without requiring Send.
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(run_analysis(path)));
        let _ = tx.send(result);
    });

    rx.recv().map_err(|e| e.to_string())?.map(Some)
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

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(OllamaState(Mutex::new(None)))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_and_analyze,
            start_ollama,
            stop_ollama,
            ollama_running,
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
