// Prevent a console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod duckdb_source;
mod helpers;
mod session_db;
mod state;

use commands::terminal::PtyState;
use commands::*;
use state::{CurrentScriptState, DslCacheState, OllamaState, ProjectsState};
use tauri::Manager;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(OllamaState(std::sync::Mutex::new(None)))
        .manage(ProjectsState(std::sync::Arc::new(std::sync::Mutex::new(None))))
        .manage(DslCacheState::default())
        .manage(PtyState::default())
        .manage(CurrentScriptState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_and_analyze,
            start_ollama,
            stop_ollama,
            ollama_running,
            check_model,
            pull_model,
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
            update_pbc_item,
            update_pbc_item_fields,
            toggle_pbc_item_approved,
            get_pbc_list_approved,
            set_pbc_list_approved,
            export_audit_plan_pdf,
            export_audit_plan_docx,
            export_pbc_pdf,
            export_pbc_docx,
            add_data_file,
            delete_project_file,
            get_data_file_headers,
            import_data_file,
            save_column_mappings,
            get_column_mappings,
            list_session_imports,
            get_session_rows,
            get_session_rows_paged,
            delete_session_import,
            get_session_schemas,
            save_tab_order,
            detect_data_relationships,
            save_dsl_script,
            list_dsl_scripts,
            clear_dsl_scripts,
            run_dsl_script,
            run_all_dsl_scripts,
            invalidate_dsl_cache,
            list_test_results,
            get_column_distribution,
            update_dsl_script,
            rename_dsl_script,
            delete_dsl_script,
            open_data_preview_window,
            open_dsl_results_window,
            get_settings,
            save_settings,
            open_settings_window,
            close_settings_window,
            export_project_vrd,
            import_project_vrd,
            save_project_vrd,
            pick_and_open_vrd,
            pty_create,
            pty_write,
            pty_resize,
            pty_kill,
            pty_update_scripts,
            pty_set_current_script,
            check_for_update,
            download_update,
            install_update,
        ])
        .setup(|app| {
            // Auto-start Ollama when the desktop app launches.
            // Skip if already running externally (e.g. Windows service).
            let state = app.state::<OllamaState>();
            if vinrouge::ollama::port_in_use(11434) {
                eprintln!("[ollama] already running on port 11434, skipping spawn");
            } else {
                match vinrouge::ollama::find_binary() {
                    Err(e) => eprintln!("[ollama] binary not found: {e}"),
                    Ok(binary) => {
                        eprintln!("[ollama] found binary: {}", binary.display());
                        let mut cmd = std::process::Command::new(binary);
                        cmd.arg("serve");
                        #[cfg(target_os = "windows")]
                        helpers::NoConsole::no_console(&mut cmd);
                        if let Some(dir) = vinrouge::ollama::resolve_models_dir(None) {
                            eprintln!("[ollama] OLLAMA_MODELS={dir}");
                            cmd.env("OLLAMA_MODELS", dir);
                        }
                        cmd.env("OLLAMA_ORIGINS", "*");
                        match cmd.spawn() {
                            Ok(child) => {
                                eprintln!("[ollama] started (pid {})", child.id());
                                *state.0.lock().unwrap() = Some(child);
                            }
                            Err(e) => eprintln!("[ollama] failed to spawn: {e}"),
                        }
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
