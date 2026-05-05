use std::collections::HashMap;
use std::sync::Arc;
use crate::components::{DataGrid, SettingsModal};
use crate::components::data_grid::open_data_window;
use crate::file_analysis::{analyze_bytes, read_file_bytes};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::ask_ai;
use crate::storage::sync_settings_from_tauri;
use vinrouge::audit_prompts::DSL_LANGUAGE_REFERENCE;
use crate::types::{DslScript, Project, ProjectFile, SessionSchema};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

const EDITOR_ID: &str = "studio-dsl-editor";

mod dsl_results;
use dsl_results::{render_dsl_result, count_results};

fn call_dsl(name: &str, args: &[&str]) {
    let Some(window) = web_sys::window() else { return };
    let Ok(f) = js_sys::Reflect::get(&window, &JsValue::from_str(name)) else { return };
    let Ok(f) = f.dyn_into::<js_sys::Function>() else { return };
    match args {
        [a] => { let _ = f.call1(&JsValue::UNDEFINED, &JsValue::from_str(a)); }
        [a, b] => { let _ = f.call2(&JsValue::UNDEFINED, &JsValue::from_str(a), &JsValue::from_str(b)); }
        _ => {}
    }
}

fn dsl_get_value() -> String {
    let Some(window) = web_sys::window() else { return String::new() };
    let Ok(f) = js_sys::Reflect::get(&window, &JsValue::from_str("vrDslGetValue")) else { return String::new() };
    let Ok(f) = f.dyn_into::<js_sys::Function>() else { return String::new() };
    f.call1(&JsValue::UNDEFINED, &JsValue::from_str(EDITOR_ID))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

fn dsl_init(code: &str) {
    call_dsl("vrDslInit", &[EDITOR_ID, code]);
}

fn dsl_set_schema(schema_json: &str) {
    call_dsl("vrDslSetSchema", &[schema_json]);
}

fn dsl_set(code: &str) {
    call_dsl("vrDslSetValue", &[EDITOR_ID, code]);
}

fn dsl_destroy() {
    call_dsl("vrDslDestroy", &[EDITOR_ID]);
}

fn dsl_refresh() {
    call_dsl("vrDslRefresh", &[EDITOR_ID]);
}

fn parse_code_blocks(text: &str) -> Vec<(bool, String)> {
    let mut segments = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("```") {
        if start > 0 {
            segments.push((false, remaining[..start].to_string()));
        }
        remaining = &remaining[start + 3..];
        // Skip optional language tag on the opening line
        if let Some(nl) = remaining.find('\n') {
            remaining = &remaining[nl + 1..];
        }
        if let Some(end) = remaining.find("```") {
            segments.push((true, remaining[..end].trim_end().to_string()));
            remaining = &remaining[end + 3..];
            if remaining.starts_with('\n') { remaining = &remaining[1..]; }
        } else {
            segments.push((true, remaining.to_string()));
            remaining = "";
        }
    }
    if !remaining.is_empty() {
        segments.push((false, remaining.to_string()));
    }
    segments
}

#[derive(Clone, PartialEq)]
enum SidebarMode {
    Projects,
    Scripts,
}

#[derive(Clone, PartialEq)]
enum StudioPanel {
    Empty,
    Creating,
    ScriptList,
    Editor,
}

#[component]
pub fn StudioView() -> impl IntoView {
    // ── Core state ────────────────────────────────────────────────────────────
    let projects: RwSignal<Vec<Project>> = RwSignal::new(vec![]);
    let active_project: RwSignal<Option<Project>> = RwSignal::new(None);
    let scripts: RwSignal<Vec<DslScript>> = RwSignal::new(vec![]);
    let active_script: RwSignal<Option<DslScript>> = RwSignal::new(None);
    let panel: RwSignal<StudioPanel> = RwSignal::new(StudioPanel::Empty);
    let sidebar_mode: RwSignal<SidebarMode> = RwSignal::new(SidebarMode::Projects);
    let sidebar_collapsed: RwSignal<bool> = RwSignal::new(false);
    let status: RwSignal<String> = RwSignal::new(String::new());
    let search: RwSignal<String> = RwSignal::new(String::new());

    // ── Create project state ──────────────────────────────────────────────────
    let new_name: RwSignal<String> = RwSignal::new(String::new());
    let creating: RwSignal<bool> = RwSignal::new(false);

    // ── Rename state ──────────────────────────────────────────────────────────
    let renaming_id: RwSignal<Option<String>> = RwSignal::new(None);
    let rename_value: RwSignal<String> = RwSignal::new(String::new());

    // ── DSL editor state ──────────────────────────────────────────────────────
    let dsl_code: RwSignal<String> = RwSignal::new(String::new());
    let dsl_output: RwSignal<String> = RwSignal::new(String::new());
    let dsl_running: RwSignal<bool> = RwSignal::new(false);
    let script_label: RwSignal<String> = RwSignal::new(String::new());

    // ── Bottom panel tab ──────────────────────────────────────────────────────
    let bottom_tab: RwSignal<&'static str> = RwSignal::new("output");

    // ── AI assistant state ────────────────────────────────────────────────────
    #[derive(Clone)]
    struct AiMsg { role: &'static str, text: String }
    let ai_messages: RwSignal<Vec<AiMsg>> = RwSignal::new(vec![]);
    let ai_input: RwSignal<String> = RwSignal::new(String::new());
    let ai_loading: RwSignal<bool> = RwSignal::new(false);

    // ── Settings modal state ──────────────────────────────────────────────────
    let settings_open: RwSignal<bool> = RwSignal::new(false);

    // ── Data panel state ──────────────────────────────────────────────────────
    let data_open: RwSignal<bool> = RwSignal::new(false);
    let popout_loading: RwSignal<bool> = RwSignal::new(false);
    let session_schemas: RwSignal<Vec<SessionSchema>> = RwSignal::new(vec![]);
    let active_schema_idx: RwSignal<usize> = RwSignal::new(0);
    let data_uploading: RwSignal<bool> = RwSignal::new(false);
    let preview_cols: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let preview_rows: RwSignal<Vec<Vec<String>>> = RwSignal::new(vec![]);
    let selected_cell: RwSignal<Option<(usize, usize)>> = RwSignal::new(None);

    // ── Load ALL rows for a table into the data panel ─────────────────────────
    let load_table_by_schema = move |s: SessionSchema| {
        let id = s.import_id.clone();
        selected_cell.set(None);
        preview_cols.set(vec![]);
        preview_rows.set(vec![]);
        let fallback_cols = s.columns.clone();
        spawn_local(async move {
            if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                "get_session_rows",
                serde_json::json!({ "importId": id }),
            ).await {
                let cols: Vec<String> = if raw.is_empty() {
                    fallback_cols
                } else {
                    let mut c: Vec<String> = raw[0].keys().cloned().collect();
                    c.sort();
                    c
                };
                let rows = raw.into_iter().map(|row| {
                    cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                }).collect();
                preview_cols.set(cols);
                preview_rows.set(rows);
            }
        });
    };

    let load_schemas = move || {
        spawn_local(async move {
            if let Ok(schemas) = tauri_invoke::<Vec<SessionSchema>>("get_session_schemas").await {
                if let Some(first) = schemas.first().cloned() {
                    active_schema_idx.set(0);
                    load_table_by_schema(first);
                } else {
                    preview_cols.set(vec![]);
                    preview_rows.set(vec![]);
                }
                // Keep DSL IntelliSense in sync with the latest imported tables
                if let Ok(json) = serde_json::to_string(&schemas) {
                    dsl_set_schema(&json);
                }
                session_schemas.set(schemas);
            }
        });
    };

    let upload_data_file = move |file: web_sys::File| {
        let name = file.name();
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        if !matches!(ext.as_str(), "csv" | "xlsx" | "xls") {
            status.set(format!("Unsupported file type (.{ext}) — drop a CSV or Excel file"));
            return;
        }
        data_uploading.set(true);
        spawn_local(async move {
            let bytes = match read_file_bytes(&file).await {
                Ok(b) => b,
                Err(e) => {
                    status.set(format!("Could not read file: {e:?}"));
                    data_uploading.set(false);
                    return;
                }
            };
            let columns = match analyze_bytes(bytes.clone(), &name).await {
                Ok(r) => {
                    let mut seen = std::collections::HashSet::new();
                    r.tables.into_iter()
                        .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                        .filter(|h| { let t = h.trim(); !t.is_empty() && seen.insert(t.to_string()) })
                        .collect::<Vec<_>>()
                }
                Err(_) => vec![],
            };
            let file_id = match tauri_invoke_args::<ProjectFile>(
                "add_data_file",
                serde_json::json!({ "name": name, "bytes": bytes }),
            ).await {
                Ok(pf) => pf.id,
                Err(e) => {
                    status.set(format!("Upload error: {e}"));
                    data_uploading.set(false);
                    return;
                }
            };
            let mappings: Vec<(String, String)> = columns.iter().map(|c| (c.clone(), c.clone())).collect();
            let _ = tauri_invoke_args::<serde_json::Value>(
                "import_data_file",
                serde_json::json!({ "fileId": file_id, "mappings": mappings, "sheet": null }),
            ).await;
            load_schemas();
            // Refresh IntelliSense schema too
            if let Ok(sj) = tauri_invoke::<serde_json::Value>("get_session_schemas").await {
                if let Ok(json) = serde_json::to_string(&sj) {
                    dsl_set_schema(&json);
                }
            }
            data_uploading.set(false);
        });
    };

    // ── Load projects on mount ────────────────────────────────────────────────
    spawn_local(async move {
        // Sync desktop settings from Tauri backend into localStorage cache
        sync_settings_from_tauri().await;

        match tauri_invoke::<Vec<Project>>("list_projects").await {
            Ok(list) => projects.set(list),
            Err(e) => status.set(format!("Failed to load projects: {e}")),
        }
        // Eagerly prime the DSL IntelliSense schema in case a project is already
        // active on the backend from a previous session.
        if let Ok(schemas) = tauri_invoke::<Vec<SessionSchema>>("get_session_schemas").await {
            if let Ok(json) = serde_json::to_string(&schemas) {
                dsl_set_schema(&json);
            }
        }
    });

    // ── Listen for settings changes from native settings window ───────────────
    if crate::ipc::is_tauri() {
        spawn_local(async move {
            let _ = crate::ipc::tauri_listen_settings_changed(|| {
                spawn_local(async move {
                    sync_settings_from_tauri().await;
                });
            }).await;
        });
    }

    // ── Load scripts for active project ───────────────────────────────────────
    let load_scripts = move || {
        spawn_local(async move {
            match tauri_invoke::<Vec<DslScript>>("list_dsl_scripts").await {
                Ok(list) => scripts.set(list),
                Err(e) => status.set(format!("Failed to load scripts: {e}")),
            }
        });
    };

    // ── Rename a script ───────────────────────────────────────────────────────
    let do_rename = move |id: String| {
        let new_label = rename_value.get().trim().to_string();
        if new_label.is_empty() {
            renaming_id.set(None);
            return;
        }
        renaming_id.set(None);
        let label_for_tab = new_label.clone();
        spawn_local(async move {
            let args = serde_json::json!({ "scriptId": id, "label": new_label });
            match tauri_invoke_args::<()>("rename_dsl_script", args).await {
                Ok(_) => {
                    // Update the editor tab label if this is the active script
                    if active_script.get().as_ref().map(|s| s.id == id).unwrap_or(false) {
                        script_label.set(label_for_tab.clone());
                        active_script.update(|s| {
                            if let Some(s) = s { s.label = label_for_tab; }
                        });
                    }
                    load_scripts();
                }
                Err(e) => status.set(format!("Rename failed: {e}")),
            }
        });
    };

    // ── Open a project ────────────────────────────────────────────────────────
    let open_project = move |path: String| {
        spawn_local(async move {
            match tauri_invoke_args::<Project>("open_project", serde_json::json!({ "path": path })).await {
                Ok(p) => {
                    active_project.set(Some(p));
                    active_script.set(None);
                    dsl_code.set(String::new());
                    dsl_output.set(String::new());
                    // Reset data panel so it shows the new project's data
                    session_schemas.set(vec![]);
                    preview_cols.set(vec![]);
                    preview_rows.set(vec![]);
                    selected_cell.set(None);
                    data_open.set(false);
                    sidebar_mode.set(SidebarMode::Scripts);
                    panel.set(StudioPanel::ScriptList);
                    search.set(String::new());
                    load_scripts();
                    // Load session schemas for IntelliSense
                    if let Ok(schemas) = tauri_invoke::<serde_json::Value>("get_session_schemas").await {
                        if let Ok(json) = serde_json::to_string(&schemas) {
                            dsl_set_schema(&json);
                        }
                    }
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Open a script into the editor ─────────────────────────────────────────
    let open_script = move |script: DslScript| {
        let code = script.script_text.clone();
        script_label.set(script.label.clone());
        dsl_output.set(String::new());
        active_script.set(Some(script));
        if panel.get() == StudioPanel::Editor {
            // Editor already mounted — just update value
            dsl_set(&code);
        } else {
            // Editor will mount; init after next frame
            dsl_code.set(code.clone());
            panel.set(StudioPanel::Editor);
            let code2 = code.clone();
            let _ = web_sys::window().unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &wasm_bindgen::closure::Closure::once_into_js(move || {
                        dsl_init(&code2);
                    }).unchecked_into(),
                    50,
                );
        }
    };

    // ── Create project ────────────────────────────────────────────────────────
    let do_create = move || {
        let name = new_name.get();
        if name.trim().is_empty() || creating.get() {
            return;
        }
        creating.set(true);
        let args = serde_json::json!({
            "name":          name,
            "saveDir":       Option::<String>::None,
            "client":        "",
            "engagementRef": "",
            "periodStart":   "",
            "periodEnd":     "",
            "reportDue":     "",
            "auditType":     "Compliance",
            "notes":         "",
            "standards":     Vec::<String>::new(),
            "scope":         "",
            "materiality":   "",
            "riskFramework": "High / Medium / Low",
        });
        spawn_local(async move {
            match tauri_invoke_args::<Project>("create_project", args).await {
                Ok(p) => {
                    // Switch backend active-project context so IPC calls (schemas,
                    // scripts, data files) all target the newly created project.
                    let _ = tauri_invoke_args::<Project>(
                        "open_project",
                        serde_json::json!({ "path": p.path }),
                    ).await;
                    new_name.set(String::new());
                    if let Ok(list) = tauri_invoke::<Vec<Project>>("list_projects").await {
                        projects.set(list);
                    }
                    active_project.set(Some(p));
                    active_script.set(None);
                    scripts.set(vec![]);
                    dsl_code.set(String::new());
                    dsl_output.set(String::new());
                    // Clear any stale data-panel state from the previous project
                    session_schemas.set(vec![]);
                    preview_cols.set(vec![]);
                    preview_rows.set(vec![]);
                    selected_cell.set(None);
                    sidebar_mode.set(SidebarMode::Scripts);
                    panel.set(StudioPanel::ScriptList);
                    search.set(String::new());
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
            creating.set(false);
        });
    };
    let on_create = move |_: web_sys::MouseEvent| do_create();

    // ── New script ────────────────────────────────────────────────────────────
    let new_script = move || {
        let label = format!("Script {}", scripts.get().len() + 1);
        let args = serde_json::json!({
            "controlId":  "",
            "controlRef": "",
            "label":      label,
            "scriptText": "",
        });
        spawn_local(async move {
            match tauri_invoke_args::<DslScript>("save_dsl_script", args).await {
                Ok(s) => {
                    load_scripts();
                    open_script(s);
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Save current script text ──────────────────────────────────────────────
    let save_script = move || {
        if let Some(s) = active_script.get() {
            let code = dsl_get_value();
            let args = serde_json::json!({ "scriptId": s.id, "scriptText": code });
            spawn_local(async move {
                if let Err(e) = tauri_invoke_args::<()>("update_dsl_script", args).await {
                    status.set(format!("Save error: {e}"));
                } else {
                    load_scripts();
                }
            });
        }
    };

    // ── Run DSL (save first so backend sees latest text) ──────────────────────
    let on_run = move |_: web_sys::MouseEvent| {
        if let Some(s) = active_script.get() {
            if dsl_running.get() { return; }
            // Persist latest editor content before running
            let code = dsl_get_value();
            let save_args = serde_json::json!({ "scriptId": s.id.clone(), "scriptText": code });
            dsl_running.set(true);
            dsl_output.set("Saving & running…".into());
            let script_id = s.id.clone();
            spawn_local(async move {
                let _ = tauri_invoke_args::<()>("update_dsl_script", save_args).await;
                let args = serde_json::json!({ "scriptId": script_id });
                match tauri_invoke_args::<serde_json::Value>("run_dsl_script", args).await {
                    Ok(v) => dsl_output.set(
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                    ),
                    Err(e) => dsl_output.set(format!("Error: {e}")),
                }
                dsl_running.set(false);
            });
        }
    };

    // ── AI send ───────────────────────────────────────────────────────────────
    let do_ai_send = move || {
        let q = ai_input.get();
        if q.trim().is_empty() || ai_loading.get() { return; }
        ai_loading.set(true);
        ai_input.set(String::new());

        // Build context: always include the full DSL language reference so the
        // LLM knows every available function, and add the current script if open.
        let script_section = if let Some(s) = active_script.get() {
            format!(
                "CURRENT SCRIPT (label: {}):\n```\n{}\n```\n\n",
                s.label,
                dsl_get_value()
            )
        } else {
            String::new()
        };
        let ctx = format!(
            "You are a VinRouge DSL assistant. You can both explain the DSL AND write \
             complete scripts for the user. When writing any DSL code — whether a full \
             script or a short snippet — always wrap it in a triple-backtick code block \
             so the user can insert it directly into the editor with one click.\n\n\
             {DSL_LANGUAGE_REFERENCE}\
             {script_section}\
             Answer concisely. When writing DSL, use only the functions documented above."
        );

        ai_messages.update(|v| v.push(AiMsg { role: "user", text: q.clone() }));

        spawn_local(async move {
            let reply = match ask_ai(&ctx, &q).await {
                Ok(r) => r,
                Err(e) => format!("Error: {e}"),
            };
            ai_messages.update(|v| v.push(AiMsg { role: "assistant", text: reply }));
            ai_loading.set(false);
        });
    };
    let on_ai_send = move |_: web_sys::MouseEvent| do_ai_send();

    view! {
        <div class="studio-shell">

            // ── Left sidebar ──────────────────────────────────────────────────
            <div class=move || if sidebar_collapsed.get() {
                "studio-sidebar studio-sidebar--collapsed"
            } else {
                "studio-sidebar"
            }>

                <div class="studio-sidebar-header">
                    {move || (!sidebar_collapsed.get()).then(|| {
                        if sidebar_mode.get() == SidebarMode::Scripts {
                            let name = active_project.get()
                                .as_ref().map(|p| p.name.clone())
                                .unwrap_or_default();
                            view! {
                                <button
                                    class="studio-back-btn"
                                    title="Back to projects"
                                    on:click=move |_| {
                                        dsl_destroy();
                                        active_project.set(None);
                                        active_script.set(None);
                                        scripts.set(vec![]);
                                        sidebar_mode.set(SidebarMode::Projects);
                                        panel.set(StudioPanel::Empty);
                                        search.set(String::new());
                                    }
                                >
                                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                        <path d="M8 2L4 6l4 4" stroke="currentColor" stroke-width="1.3"
                                            stroke-linecap="round" stroke-linejoin="round"/>
                                    </svg>
                                </button>
                                <span class="studio-sidebar-title" style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
                                    {name}
                                </span>
                            }.into_any()
                        } else {
                            view! {
                                <span class="studio-sidebar-title">"STUDIO"</span>
                            }.into_any()
                        }
                    })}
                    <button
                        class="studio-collapse-btn"
                        title="Settings"
                        on:click=move |_| {
                            if crate::ipc::is_tauri() {
                                spawn_local(async move {
                                    let _ = crate::ipc::tauri_invoke::<()>("open_settings_window").await;
                                });
                            } else {
                                settings_open.set(true);
                            }
                        }
                    >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="12" cy="12" r="3"/>
                            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
                        </svg>
                    </button>
                    <button
                        class="studio-collapse-btn"
                        title=move || if sidebar_collapsed.get() { "Expand" } else { "Collapse" }
                        on:click=move |_| sidebar_collapsed.update(|v| *v = !*v)
                    >
                        {move || if sidebar_collapsed.get() {
                            view! {
                                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                                    <path d="M5 3l4 4-4 4" stroke="currentColor" stroke-width="1.3"
                                        stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                                    <path d="M9 3L5 7l4 4" stroke="currentColor" stroke-width="1.3"
                                        stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                            }.into_any()
                        }}
                    </button>
                </div>

                // ── Collapsed icons ───────────────────────────────────────────
                {move || sidebar_collapsed.get().then(|| view! {
                    <div class="studio-collapsed-icons">
                        <button class="studio-icon-btn" title="New project"
                            on:click=move |_| {
                                sidebar_collapsed.set(false);
                                panel.set(StudioPanel::Creating);
                            }
                        >
                            <svg width="13" height="13" viewBox="0 0 12 12" fill="none">
                                <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                            </svg>
                        </button>
                    </div>
                })}

                // ── Expanded: projects list ───────────────────────────────────
                {move || (!sidebar_collapsed.get() && sidebar_mode.get() == SidebarMode::Projects).then(|| view! {
                    <>
                        <div class="studio-sidebar-actions">
                            <button
                                class=move || if panel.get() == StudioPanel::Creating {
                                    "studio-new-btn studio-new-btn--active"
                                } else { "studio-new-btn" }
                                on:click=move |_| {
                                    new_name.set(String::new());
                                    panel.set(StudioPanel::Creating);
                                }
                            >
                                <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                    <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                                </svg>
                                "New project"
                            </button>
                        </div>

                        <div class="proj-search-box" style="margin:0 10px 8px">
                            <svg class="proj-search-icon" width="13" height="13" viewBox="0 0 13 13" fill="none">
                                <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.2"/>
                                <path d="M9 9l2.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                            </svg>
                            <input class="proj-search-input" type="text" placeholder="Search projects..."
                                prop:value=move || search.get()
                                on:input=move |ev| search.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="proj-list">
                            {move || {
                                let q = search.get().to_lowercase();
                                projects.get().into_iter()
                                    .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q))
                                    .map(|p| {
                                        let path = p.path.clone();
                                        let name = p.name.clone();
                                        let date = p.created_at.get(..10).unwrap_or("").to_string();
                                        view! {
                                            <div class="proj-list-item"
                                                on:click=move |_| open_project(path.clone())
                                            >
                                                <div class="proj-item-row1">
                                                    <span class="proj-item-name">{name}</span>
                                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none"
                                                        style="color:var(--w-text-4);flex-shrink:0">
                                                        <path d="M4 2l4 4-4 4" stroke="currentColor"
                                                            stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                                                    </svg>
                                                </div>
                                                <div class="proj-item-meta">{date}</div>
                                            </div>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </div>
                    </>
                })}

                // ── Expanded: scripts list ────────────────────────────────────
                {move || (!sidebar_collapsed.get() && sidebar_mode.get() == SidebarMode::Scripts).then(|| view! {
                    <>
                        <div class="studio-sidebar-actions">
                            <button class="studio-new-btn" on:click=move |_| new_script()>
                                <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                    <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                                </svg>
                                "New script"
                            </button>
                        </div>

                        <div class="proj-search-box" style="margin:0 10px 8px">
                            <svg class="proj-search-icon" width="13" height="13" viewBox="0 0 13 13" fill="none">
                                <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.2"/>
                                <path d="M9 9l2.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                            </svg>
                            <input class="proj-search-input" type="text" placeholder="Search scripts..."
                                prop:value=move || search.get()
                                on:input=move |ev| search.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="proj-list" style="flex:1">
                            {move || {
                                let q = search.get().to_lowercase();
                                let list = scripts.get();
                                if list.is_empty() {
                                    return view! {
                                        <div class="studio-scripts-empty">
                                            "No scripts yet — create one above."
                                        </div>
                                    }.into_any();
                                }
                                list.into_iter()
                                    .filter(|s| q.is_empty() || s.label.to_lowercase().contains(&q))
                                    .map(|s| {
                                        let s_clone  = s.clone();
                                        // Arc lets multiple closures share the same string cheaply
                                        let sid      = std::sync::Arc::new(s.id.clone());
                                        let label    = std::sync::Arc::new(s.label.clone());
                                        let sid_del  = Arc::clone(&sid);
                                        let date     = s.created_at.get(..10).unwrap_or("").to_string();
                                        view! {
                                            <div
                                                class={
                                                    let sid = Arc::clone(&sid);
                                                    move || {
                                                        let active = active_script.get()
                                                            .as_ref().map(|a| a.id.as_str() == sid.as_str()).unwrap_or(false);
                                                        if active { "proj-list-item proj-list-item--active" }
                                                        else      { "proj-list-item" }
                                                    }
                                                }
                                                on:click={
                                                    let sid = Arc::clone(&sid);
                                                    move |_| {
                                                        if renaming_id.get().as_deref() != Some(sid.as_str()) {
                                                            open_script(s_clone.clone());
                                                        }
                                                    }
                                                }
                                            >
                                                <div class="proj-item-row1">
                                                    <span class="studio-script-icon">
                                                        <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                                            <rect x="1.5" y="1.5" width="9" height="9" rx="1.5"
                                                                stroke="currentColor" stroke-width="1.1"/>
                                                            <path d="M3.5 4.5h5M3.5 6h3.5M3.5 7.5h4"
                                                                stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
                                                        </svg>
                                                    </span>

                                                    // Label or inline rename input
                                                    {{
                                                        let sid   = Arc::clone(&sid);
                                                        let label = Arc::clone(&label);
                                                        move || {
                                                            let is_renaming = renaming_id.get().as_deref() == Some(sid.as_str());
                                                            if is_renaming {
                                                                let sid_kd = Arc::clone(&sid);
                                                                let sid_bl = Arc::clone(&sid);
                                                                view! {
                                                                    <input
                                                                        class="proj-rename-input"
                                                                        type="text"
                                                                        prop:value=move || rename_value.get()
                                                                        on:input=move |ev| rename_value.set(event_target_value(&ev))
                                                                        on:keydown=move |ev| {
                                                                            if ev.key() == "Enter"  { do_rename(sid_kd.to_string()); }
                                                                            if ev.key() == "Escape" { renaming_id.set(None); }
                                                                        }
                                                                        on:blur=move |_| do_rename(sid_bl.to_string())
                                                                        autofocus=true
                                                                        on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()
                                                                    />
                                                                }.into_any()
                                                            } else {
                                                                let sid_db   = Arc::clone(&sid);
                                                                let label_db = Arc::clone(&label);
                                                                let label_txt = label.to_string();
                                                                view! {
                                                                    <span
                                                                        class="proj-item-name"
                                                                        title="Double-click to rename"
                                                                        on:dblclick=move |ev| {
                                                                            ev.stop_propagation();
                                                                            rename_value.set(label_db.to_string());
                                                                            renaming_id.set(Some(sid_db.to_string()));
                                                                        }
                                                                    >{label_txt}</span>
                                                                }.into_any()
                                                            }
                                                        }
                                                    }}

                                                    // Pencil rename button (hidden while renaming)
                                                    {{
                                                        let sid   = Arc::clone(&sid);
                                                        let label = Arc::clone(&label);
                                                        move || {
                                                            if renaming_id.get().as_deref() == Some(sid.as_str()) {
                                                                return None;
                                                            }
                                                            let sid_p   = Arc::clone(&sid);
                                                            let label_p = Arc::clone(&label);
                                                            Some(view! {
                                                                <button
                                                                    class="proj-rename-btn"
                                                                    title="Rename"
                                                                    on:click=move |ev| {
                                                                        ev.stop_propagation();
                                                                        rename_value.set(label_p.to_string());
                                                                        renaming_id.set(Some(sid_p.to_string()));
                                                                    }
                                                                >
                                                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                                                        <path d="M8.5 1.5l2 2-6 6H2.5v-2l6-6z"
                                                                            stroke="currentColor" stroke-width="1.1"
                                                                            stroke-linejoin="round"/>
                                                                    </svg>
                                                                </button>
                                                            })
                                                        }
                                                    }}

                                                    <button
                                                        class="proj-delete-btn"
                                                        title="Delete script"
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            let id = sid_del.to_string();
                                                            spawn_local(async move {
                                                                let args = serde_json::json!({ "scriptId": id.clone() });
                                                                match tauri_invoke_args::<()>("delete_dsl_script", args).await {
                                                                    Ok(_) => {
                                                                        if active_script.get().as_ref().map(|s| s.id == id).unwrap_or(false) {
                                                                            dsl_destroy();
                                                                            active_script.set(None);
                                                                            panel.set(StudioPanel::ScriptList);
                                                                        }
                                                                        load_scripts();
                                                                    }
                                                                    Err(e) => status.set(format!("Delete failed: {e}")),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                                            <path d="M3 3l6 6M9 3l-6 6"
                                                                stroke="currentColor" stroke-width="1.3"
                                                                stroke-linecap="round"/>
                                                        </svg>
                                                    </button>
                                                </div>
                                                <div class="proj-item-meta">{date}</div>
                                            </div>
                                        }
                                    })
                                    .collect_view()
                                    .into_any()
                            }}
                        </div>
                    </>
                })}
            </div>

            // ── Main area ─────────────────────────────────────────────────────
            <div class="studio-main">

                {move || {
                    let s = status.get();
                    (!s.is_empty()).then(|| view! { <div class="proj-status-toast">{s}</div> })
                }}

                // ── Empty state ───────────────────────────────────────────────
                {move || (panel.get() == StudioPanel::Empty).then(|| view! {
                    <div class="proj-empty-state">
                        <div class="proj-empty-icon">
                            <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
                                <rect x="4" y="4" width="18" height="18" rx="2"
                                    stroke="currentColor" stroke-width="1.3"/>
                                <path d="M9 10h8M9 13h5M9 16h6"
                                    stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                            </svg>
                        </div>
                        <div class="proj-empty-title">"DSL Studio"</div>
                        <div class="proj-empty-sub">
                            "Select a project from the sidebar to open its scripts, or create a new project to get started."
                        </div>
                        <button class="wiz-btn-primary" on:click=move |_| {
                            new_name.set(String::new());
                            panel.set(StudioPanel::Creating);
                        }>"New project"</button>
                    </div>
                })}

                // ── Create project panel ──────────────────────────────────────
                {move || (panel.get() == StudioPanel::Creating).then(|| view! {
                    <div class="studio-create-panel">
                        <div class="studio-create-inner">
                            <div class="studio-create-icon">
                                <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
                                    <rect x="3" y="3" width="18" height="18" rx="3"
                                        stroke="currentColor" stroke-width="1.4"/>
                                    <path d="M12 8v8M8 12h8"
                                        stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                                </svg>
                            </div>
                            <div class="studio-create-title">"New DSL project"</div>
                            <div class="studio-create-sub">
                                "Give your project a name to get started."
                            </div>
                            <div class="studio-create-form">
                                <input
                                    class="wiz-input"
                                    type="text"
                                    placeholder="Project name"
                                    prop:value=move || new_name.get()
                                    on:input=move |ev| new_name.set(event_target_value(&ev))
                                    on:keydown=move |ev| { if ev.key() == "Enter" { do_create(); } }
                                />
                                <div class="studio-create-actions">
                                    <button class="wiz-btn-ghost"
                                        on:click=move |_| panel.set(StudioPanel::Empty)>"Cancel"</button>
                                    <button class="wiz-btn-primary"
                                        disabled=move || new_name.get().trim().is_empty() || creating.get()
                                        on:click=on_create>
                                        {move || if creating.get() { "Creating…" } else { "Create & open" }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                })}

                // ── Script list (project open, no script selected) ─────────────
                {move || (panel.get() == StudioPanel::ScriptList).then(|| {
                    let proj_name = active_project.get()
                        .as_ref().map(|p| p.name.clone())
                        .unwrap_or_default();
                    view! {
                        <div class="studio-scriptlist-shell">
                            <div class="studio-scriptlist-header">
                                <div>
                                    <div class="studio-scriptlist-title">{proj_name}</div>
                                    <div class="studio-scriptlist-sub">
                                        {move || {
                                            let n = scripts.get().len();
                                            if n == 0 { "No scripts yet".to_string() }
                                            else { format!("{} script{}", n, if n == 1 { "" } else { "s" }) }
                                        }}
                                    </div>
                                </div>
                                <button class="wiz-btn-primary" on:click=move |_| new_script()>
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <path d="M6 1v10M1 6h10" stroke="currentColor"
                                            stroke-width="1.3" stroke-linecap="round"/>
                                    </svg>
                                    "New script"
                                </button>
                            </div>

                            {move || {
                                let list = scripts.get();
                                if list.is_empty() {
                                    view! {
                                        <div class="studio-scriptlist-empty">
                                            <svg width="32" height="32" viewBox="0 0 32 32" fill="none"
                                                style="color:var(--w-text-4);margin-bottom:10px">
                                                <rect x="5" y="5" width="22" height="22" rx="3"
                                                    stroke="currentColor" stroke-width="1.4"/>
                                                <path d="M10 12h12M10 16h8M10 20h10"
                                                    stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                                            </svg>
                                            <div>"No DSL scripts in this project yet."</div>
                                            <div style="font-size:12px;color:var(--w-text-4);margin-top:4px">
                                                "Click \"New script\" to create your first one."
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="studio-scriptlist-grid">
                                            {list.into_iter().map(|s| {
                                                let s_clone = s.clone();
                                                let label = s.label.clone();
                                                let date = s.created_at.get(..10).unwrap_or("").to_string();
                                                let preview = {
                                                    let t = s.script_text.trim();
                                                    if t.is_empty() { "— empty —".to_string() }
                                                    else {
                                                        let line = t.lines().next().unwrap_or("");
                                                        if line.len() > 80 { format!("{}…", &line[..80]) }
                                                        else { line.to_string() }
                                                    }
                                                };
                                                view! {
                                                    <div class="studio-script-card"
                                                        on:click=move |_| open_script(s_clone.clone())>
                                                        <div class="studio-script-card-icon">
                                                            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                                                                <rect x="2" y="2" width="12" height="12" rx="2"
                                                                    stroke="currentColor" stroke-width="1.2"/>
                                                                <path d="M4.5 6h7M4.5 8.5h5M4.5 11h6"
                                                                    stroke="currentColor" stroke-width="1"
                                                                    stroke-linecap="round"/>
                                                            </svg>
                                                        </div>
                                                        <div class="studio-script-card-body">
                                                            <div class="studio-script-card-label">{label}</div>
                                                            <div class="studio-script-card-preview">{preview}</div>
                                                        </div>
                                                        <div class="studio-script-card-meta">{date}</div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                    }
                })}

                // ── DSL editor (IDE layout) ───────────────────────────────────
                {move || (panel.get() == StudioPanel::Editor).then(|| view! {
                    <div class="ide-shell">

                        // ── Top bar ───────────────────────────────────────────
                        <div class="ide-topbar">
                            // Breadcrumb tab
                            <div class="ide-tab-row">
                                <div class="ide-tab ide-tab--active">
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <rect x="1.5" y="1.5" width="9" height="9" rx="1.5"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M3.5 4.5h5M3.5 6.5h3.5M3.5 8.5h4"
                                            stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
                                    </svg>
                                    {move || script_label.get()}
                                </div>
                                <div class="ide-topbar-spacer" />
                                <span class="ide-hint-tip">"Ctrl+Space — autocomplete"</span>
                                <button
                                    class=move || if data_open.get() { "ide-data-btn ide-data-btn--active" } else { "ide-data-btn" }
                                    on:click=move |_| {
                                        let opening = !data_open.get();
                                        data_open.set(opening);
                                        if opening { load_schemas(); }
                                    }
                                >
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <ellipse cx="6" cy="3" rx="4" ry="1.5"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M2 3v3c0 .83 1.79 1.5 4 1.5S10 6.83 10 6V3"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M2 6v3c0 .83 1.79 1.5 4 1.5S10 9.83 10 9V6"
                                            stroke="currentColor" stroke-width="1.1"/>
                                    </svg>
                                    "Data"
                                </button>
                                <button class="ide-save-btn" on:click=move |_| save_script()>
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <path d="M2 2h6l2 2v6a1 1 0 01-1 1H3a1 1 0 01-1-1V2z"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M4 2v3h4V2M4 8h4"
                                            stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
                                    </svg>
                                    "Save"
                                </button>
                                <button
                                    class="ide-run-btn"
                                    disabled=move || dsl_running.get()
                                    on:click=on_run
                                >
                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                        <path d="M3 2l7 4-7 4V2z"
                                            fill="currentColor" stroke="currentColor"
                                            stroke-width="0.4" stroke-linejoin="round"/>
                                    </svg>
                                    {move || if dsl_running.get() { "Running…" } else { "Run" }}
                                </button>
                            </div>
                        </div>

                        // ── Data overlay (shown when data_open) ──────────────
                        <div class="ide-data-overlay"
                            style=move || if data_open.get() { "display:flex" } else { "display:none" }
                        >
                            // Header: tabs + upload + close
                            <div class="ide-data-header">
                                <span class="ide-data-title">
                                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                        <ellipse cx="6" cy="3" rx="4" ry="1.5"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M2 3v3c0 .83 1.79 1.5 4 1.5S10 6.83 10 6V3"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M2 6v3c0 .83 1.79 1.5 4 1.5S10 9.83 10 9V6"
                                            stroke="currentColor" stroke-width="1.1"/>
                                    </svg>
                                    "Data"
                                </span>
                                // Table tabs (one per imported table)
                                {move || {
                                    let schemas = session_schemas.get();
                                    schemas.into_iter().enumerate().map(|(i, s)| {
                                        let s2 = s.clone();
                                        let label = s.table_name.clone();
                                        let rows = s.row_count;
                                        view! {
                                            <button
                                                class=move || if active_schema_idx.get() == i {
                                                    "ide-data-tab ide-data-tab--active"
                                                } else { "ide-data-tab" }
                                                on:click=move |_| {
                                                    active_schema_idx.set(i);
                                                    load_table_by_schema(s2.clone());
                                                }
                                            >
                                                {label}
                                                <span class="ide-data-tab-count">{format!("{rows}")}</span>
                                            </button>
                                        }
                                    }).collect_view()
                                }}
                                <div style="flex:1" />
                                // Pop-out: open a blank window NOW (user-gesture context),
                                // then fetch all tables and navigate the window to the data.
                                <button
                                    class="ide-data-upload-btn"
                                    title="Open all tables in a new window"
                                    disabled=move || popout_loading.get()
                                    on:click=move |_| {
                                        let schemas = session_schemas.get();
                                        if schemas.is_empty() || popout_loading.get() { return; }
                                        popout_loading.set(true);

                                        // For browser path: must open the window here, before any await,
                                        // to stay within the user-gesture context (avoids popup blocking).
                                        // For Tauri: the native command creates its own window, pass None.
                                        let pre_win: Option<web_sys::Window> = if !crate::ipc::is_tauri() {
                                            web_sys::window()
                                                .and_then(|w| w.open_with_url_and_target("about:blank", "_blank").ok().flatten())
                                        } else {
                                            None
                                        };

                                        spawn_local(async move {
                                            let mut all_tables: Vec<(String, Vec<String>, Vec<Vec<String>>)> = vec![];
                                            for s in &schemas {
                                                let id   = s.import_id.clone();
                                                let name = s.table_name.clone();
                                                let fallback_cols = s.columns.clone();
                                                if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                                                    "get_session_rows",
                                                    serde_json::json!({ "importId": id }),
                                                ).await {
                                                    let cols: Vec<String> = if raw.is_empty() {
                                                        fallback_cols
                                                    } else {
                                                        let mut c: Vec<String> = raw[0].keys().cloned().collect();
                                                        c.sort();
                                                        c
                                                    };
                                                    let rows = raw.into_iter().map(|row| {
                                                        cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                                                    }).collect();
                                                    all_tables.push((name, cols, rows));
                                                }
                                            }
                                            open_data_window(all_tables, pre_win).await;
                                            popout_loading.set(false);
                                        });
                                    }
                                >
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <path d="M5 2H2a1 1 0 00-1 1v7a1 1 0 001 1h7a1 1 0 001-1V7"
                                            stroke="currentColor" stroke-width="1.1"
                                            stroke-linecap="round" stroke-linejoin="round"/>
                                        <path d="M8 1h3v3M11 1L6 6"
                                            stroke="currentColor" stroke-width="1.1"
                                            stroke-linecap="round" stroke-linejoin="round"/>
                                    </svg>
                                    {move || if popout_loading.get() { "Opening…" } else { "Pop out" }}
                                </button>

                                // Refresh cache button
                                {move || if crate::ipc::is_tauri() { view! {
                                    <button
                                        class="ide-data-upload-btn"
                                        title="Refresh data cache — forces DuckDB to reload from storage on next script run"
                                        on:click=move |_| {
                                            leptos::task::spawn_local(async move {
                                                let _ = crate::ipc::tauri_invoke::<serde_json::Value>("invalidate_dsl_cache").await;
                                            });
                                        }
                                    >
                                        <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                            <path d="M10 6A4 4 0 1 1 6 2a4 4 0 0 1 2.83 1.17L10 2v4H6"
                                                stroke="currentColor" stroke-width="1.1"
                                                stroke-linecap="round" stroke-linejoin="round"/>
                                        </svg>
                                        "Refresh"
                                    </button>
                                }.into_any() } else { view! { <span></span> }.into_any() }}

                                // Upload button (always visible)
                                <label class="ide-data-upload-btn" title="Upload CSV or Excel file">
                                    <input
                                        type="file"
                                        accept=".csv,.xlsx,.xls"
                                        style="display:none"
                                        on:change=move |ev| {
                                            use wasm_bindgen::JsCast;
                                            if let Some(input) = ev.target()
                                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                            {
                                                if let Some(files) = input.files() {
                                                    if let Some(f) = files.get(0) {
                                                        upload_data_file(f);
                                                    }
                                                }
                                            }
                                        }
                                    />
                                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                        <path d="M6 1v7M3 4l3-3 3 3M2 9v2h8V9"
                                            stroke="currentColor" stroke-width="1.1"
                                            stroke-linecap="round" stroke-linejoin="round"/>
                                    </svg>
                                    "Upload"
                                </label>
                                // Close
                                <button class="ide-data-close" on:click=move |_| data_open.set(false)>
                                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                        <path d="M2 2l8 8M10 2l-8 8"
                                            stroke="currentColor" stroke-width="1.3"
                                            stroke-linecap="round"/>
                                    </svg>
                                </button>
                            </div>

                            // Body
                            <div class="ide-data-body">
                                {move || {
                                    if data_uploading.get() {
                                        return view! {
                                            <div class="ide-data-empty">
                                                <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
                                                    <circle cx="12" cy="12" r="10"
                                                        stroke="#3a1f25" stroke-width="2"/>
                                                    <path d="M12 2a10 10 0 0 1 10 10"
                                                        stroke="#8b1a2a" stroke-width="2"
                                                        stroke-linecap="round">
                                                        <animateTransform attributeName="transform"
                                                            type="rotate" from="0 12 12" to="360 12 12"
                                                            dur="0.8s" repeatCount="indefinite"/>
                                                    </path>
                                                </svg>
                                                "Uploading…"
                                            </div>
                                        }.into_any();
                                    }
                                    if session_schemas.get().is_empty() {
                                        // Empty state: drag-drop upload zone
                                        return view! {
                                            <div class="ide-data-upload-zone"
                                                on:dragover=|ev: web_sys::DragEvent| { ev.prevent_default(); }
                                                on:drop=move |ev: web_sys::DragEvent| {
                                                    ev.prevent_default();
                                                    if let Some(dt) = ev.data_transfer() {
                                                        if let Some(files) = dt.files() {
                                                            if let Some(f) = files.get(0) {
                                                                upload_data_file(f);
                                                            }
                                                        }
                                                    }
                                                }
                                            >
                                                <svg width="48" height="48" viewBox="0 0 48 48" fill="none" class="ide-data-zone-icon">
                                                    <rect x="4" y="10" width="40" height="30" rx="4"
                                                        stroke="currentColor" stroke-width="2"/>
                                                    <path d="M14 10V8a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v2"
                                                        stroke="currentColor" stroke-width="2"/>
                                                    <path d="M24 18v12M18 24l6-6 6 6"
                                                        stroke="currentColor" stroke-width="2"
                                                        stroke-linecap="round" stroke-linejoin="round"/>
                                                </svg>
                                                <p class="ide-data-zone-title">"Drop a CSV or Excel file here"</p>
                                                <p class="ide-data-zone-sub">".csv, .xlsx, .xls accepted"</p>
                                                <label class="ide-data-zone-btn">
                                                    <input
                                                        type="file"
                                                        accept=".csv,.xlsx,.xls"
                                                        style="display:none"
                                                        on:change=move |ev| {
                                                            use wasm_bindgen::JsCast;
                                                            if let Some(input) = ev.target()
                                                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                            {
                                                                if let Some(files) = input.files() {
                                                                    if let Some(f) = files.get(0) {
                                                                        upload_data_file(f);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    />
                                                    "Browse files"
                                                </label>
                                            </div>
                                        }.into_any();
                                    }

                                    // Spreadsheet grid
                                    let all_cols = preview_cols.get();
                                    let all_rows = preview_rows.get();
                                    if all_cols.is_empty() {
                                        return view! {
                                            <div class="ide-data-empty">"Loading…"</div>
                                        }.into_any();
                                    }

                                    view! {
                                        <div class="ide-data-grid-shell">
                                            <div class="ide-data-grid-topbar">
                                                <span class="ide-data-row-count">
                                                    {move || format!("{} rows", preview_rows.get().len())}
                                                </span>
                                            </div>
                                            <DataGrid
                                                cols=all_cols
                                                rows=all_rows
                                                selected_cell=selected_cell
                                            />
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        </div>

                        // ── Editor area (fills remaining space) ───────────────
                        <div class="ide-editor-area"
                            style=move || if data_open.get() { "display:none" } else { "" }
                        >
                            <div id="studio-dsl-editor" class="studio-cm-host" />
                        </div>

                        // ── Bottom panel ──────────────────────────────────────
                        <div class="ide-bottom-panel"
                            style=move || if data_open.get() { "display:none" } else { "" }
                        >

                            // Tab bar
                            <div class="ide-bottom-tabbar">
                                <div
                                    class=move || if bottom_tab.get() == "output" {
                                        "ide-bottom-tab ide-bottom-tab--active"
                                    } else { "ide-bottom-tab" }
                                    on:click=move |_| bottom_tab.set("output")
                                >
                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                        <rect x="1" y="1" width="10" height="10" rx="2"
                                            stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M3.5 4.5l2 2-2 2M6.5 8.5h2"
                                            stroke="currentColor" stroke-width="1"
                                            stroke-linecap="round" stroke-linejoin="round"/>
                                    </svg>
                                    "Output"
                                </div>
                                <div
                                    class=move || if bottom_tab.get() == "ai" {
                                        "ide-bottom-tab ide-bottom-tab--active"
                                    } else { "ide-bottom-tab" }
                                    on:click=move |_| bottom_tab.set("ai")
                                >
                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                        <circle cx="6" cy="6" r="5" stroke="currentColor" stroke-width="1.1"/>
                                        <path d="M4 5c0-1.1.9-2 2-2s2 .9 2 2c0 .8-.5 1.5-1.2 1.8L6.5 8h-1V7l.7-.2C6.7 6.6 7 6.3 7 6"
                                            stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>
                                        <circle cx="6" cy="9.5" r=".5" fill="currentColor"/>
                                    </svg>
                                    "AI Assistant"
                                </div>
                                <div class="ide-bottom-tabbar-spacer" />
                                // Tab-specific actions
                                {move || (bottom_tab.get() == "output" && !dsl_output.get().is_empty()).then(|| view! {
                                    <button class="ide-clear-btn"
                                        on:click=move |_| dsl_output.set(String::new())>
                                        "Clear"
                                    </button>
                                })}
                                {move || (bottom_tab.get() == "ai" && !ai_messages.get().is_empty()).then(|| view! {
                                    <button class="ide-clear-btn"
                                        on:click=move |_| ai_messages.set(vec![])>
                                        "Clear"
                                    </button>
                                })}
                            </div>

                            // Output tab — rich result rendering
                            {move || (bottom_tab.get() == "output").then(|| {
                                let raw = dsl_output.get();

                                if raw.is_empty() {
                                    return view! {
                                        <div class="ide-output-empty">
                                            "Run your script to see results here"
                                        </div>
                                    }.into_any();
                                }

                                // While saving/running show spinner line
                                if dsl_running.get() || !raw.starts_with('[') {
                                    return view! {
                                        <pre class="ide-output">{raw}</pre>
                                    }.into_any();
                                }

                                // Parse JSON result array
                                let results: Vec<serde_json::Value> = match serde_json::from_str(&raw) {
                                    Ok(v) => v,
                                    Err(_) => return view! {
                                        <pre class="ide-output">{raw}</pre>
                                    }.into_any(),
                                };

                                let (passed, failed, errors, samples, charts, screens, sections) = count_results(&results);

                                view! {
                                    <div class="ide-results">
                                        // Summary bar
                                        <div class="ide-results-summary">
                                            {(passed > 0 || failed > 0).then(|| view! {
                                                <>
                                                    {(passed > 0).then(|| view! {
                                                        <span class="ide-sum ide-sum--pass">
                                                            <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                                                <path d="M2 6l3 3 5-5" stroke="currentColor"
                                                                    stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                                                            </svg>
                                                            {format!("{passed} passed")}
                                                        </span>
                                                    })}
                                                    {(failed > 0).then(|| view! {
                                                        <span class="ide-sum ide-sum--fail">
                                                            <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                                                <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor"
                                                                    stroke-width="1.5" stroke-linecap="round"/>
                                                            </svg>
                                                            {format!("{failed} failed")}
                                                        </span>
                                                    })}
                                                </>
                                            })}
                                            {(errors > 0).then(|| view! {
                                                <span class="ide-sum ide-sum--error">
                                                    {format!("{errors} error{}", if errors == 1 { "" } else { "s" })}
                                                </span>
                                            })}
                                            {(samples > 0).then(|| view! {
                                                <span class="ide-sum ide-sum--sample">
                                                    {format!("{samples} sample{}", if samples == 1 { "" } else { "s" })}
                                                </span>
                                            })}
                                            {(charts > 0).then(|| view! {
                                                <span class="ide-sum ide-sum--chart">
                                                    {format!("{charts} chart{}", if charts == 1 { "" } else { "s" })}
                                                </span>
                                            })}
                                            {(screens > 0).then(|| view! {
                                                <span class="ide-sum ide-sum--screen">
                                                    {format!("{screens} screen{}", if screens == 1 { "" } else { "s" })}
                                                </span>
                                            })}
                                            {(sections > 0).then(|| view! {
                                                <span class="ide-sum ide-sum--section">
                                                    {format!("{sections} section{}", if sections == 1 { "" } else { "s" })}
                                                </span>
                                            })}
                                        </div>

                                        // Pop-out button for testing
                                        {
                                            let results_for_popout = results.clone();
                                            view! {
                                                <div style="display:flex;justify-content:flex-end;margin-bottom:8px;">
                                                    <button
                                                        class="ide-data-upload-btn"
                                                        title="Open results in new window for testing"
                                                        on:click=move |_| {
                                                            dsl_results::open_results_window_async(results_for_popout.clone());
                                                        }
                                                        style="padding:4px 10px;font-size:11px;"
                                                    >
                                                        <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                                            <path d="M5 2H2a1 1 0 00-1 1v7a1 1 0 001 1h7a1 1 0 001-1V7" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"/>
                                                            <path d="M8 1h3v3M11 1L6 6" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"/>
                                                        </svg>
                                                        "Pop out"
                                                    </button>
                                                </div>
                                            }.into_any()
                                        }

                                        // Per-statement results
                                        {results.into_iter().enumerate().map(|(idx, r)| render_dsl_result(idx, r)).collect::<Vec<_>>()}

                                        // Raw JSON section
                                        <div class="ide-raw-json-section">
                                            <div class="ide-raw-json-label">"JSON"</div>
                                            <pre class="ide-raw-json">{raw}</pre>
                                        </div>
                                    </div>
                                }.into_any()
                            })}

                            // AI assistant tab
                            {move || (bottom_tab.get() == "ai").then(|| view! {
                                <div class="ide-ai-panel">
                                    <div class="ide-ai-messages">
                                        {move || {
                                            let msgs = ai_messages.get();
                                            if msgs.is_empty() {
                                                return view! {
                                                    <div class="ide-ai-empty">
                                                        <div>"Ask me to explain DSL, debug your script, or write one from scratch."</div>
                                                        <div class="ide-ai-suggestions">
                                                            <button class="ide-ai-suggestion"
                                                                on:click=move |_| ai_input.set(
                                                                    "Write a script that checks for duplicate values in a key column".into()
                                                                )>"Check for duplicates"</button>
                                                            <button class="ide-ai-suggestion"
                                                                on:click=move |_| ai_input.set(
                                                                    "Write a script that validates all amount fields are positive numbers".into()
                                                                )>"Validate amounts"</button>
                                                            <button class="ide-ai-suggestion"
                                                                on:click=move |_| ai_input.set(
                                                                    "Write a script that takes a MUS sample of 30 from the transactions table".into()
                                                                )>"MUS sample"</button>
                                                        </div>
                                                    </div>
                                                }.into_any();
                                            }
                                            msgs.into_iter().map(|m| {
                                                let is_user = m.role == "user";
                                                let text = m.text.clone();
                                                view! {
                                                    <div class=if is_user { "ide-ai-msg ide-ai-msg--user" } else { "ide-ai-msg ide-ai-msg--ai" }>
                                                        <span class="ide-ai-role">
                                                            {if is_user { "You" } else { "AI" }}
                                                        </span>
                                                        {if is_user {
                                                            view! { <span class="ide-ai-text">{text}</span> }.into_any()
                                                        } else {
                                                            let segments = parse_code_blocks(&text);
                                                            view! {
                                                                <div class="ide-ai-text">
                                                                    {segments.into_iter().map(|(is_code, content)| {
                                                                        if is_code {
                                                                            let code = content.clone();
                                                                            view! {
                                                                                <div class="ide-ai-code-block">
                                                                                    <div class="ide-ai-code-header">
                                                                                        <span class="ide-ai-code-lang">"DSL"</span>
                                                                                        <button class="ide-ai-insert-btn"
                                                                                            on:click=move |_| dsl_set(&code)>
                                                                                            <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                                                                                <path d="M1 6h10M7 2l4 4-4 4"
                                                                                                    stroke="currentColor" stroke-width="1.3"
                                                                                                    stroke-linecap="round" stroke-linejoin="round"/>
                                                                                            </svg>
                                                                                            "Insert into editor"
                                                                                        </button>
                                                                                    </div>
                                                                                    <pre class="ide-ai-code-pre">{content}</pre>
                                                                                </div>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! {
                                                                                <span class="ide-ai-text-seg">{content}</span>
                                                                            }.into_any()
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            }.into_any()
                                                        }}
                                                    </div>
                                                }
                                            }).collect_view().into_any()
                                        }}
                                        {move || ai_loading.get().then(|| view! {
                                            <div class="ide-ai-msg ide-ai-msg--ai ide-ai-msg--loading">
                                                <span class="ide-ai-role">"AI"</span>
                                                <span class="ide-ai-thinking">
                                                    <span /><span /><span />
                                                </span>
                                            </div>
                                        })}
                                    </div>
                                    <div class="ide-ai-inputrow">
                                        <input
                                            class="ide-ai-input"
                                            type="text"
                                            placeholder="Ask about DSL, or say 'write a script that…'"
                                            prop:value=move || ai_input.get()
                                            on:input=move |ev| ai_input.set(event_target_value(&ev))
                                            on:keydown=move |ev| {
                                                if ev.key() == "Enter" { do_ai_send(); }
                                            }
                                        />
                                        <button
                                            class="ide-ai-send"
                                            disabled=move || ai_loading.get() || ai_input.get().trim().is_empty()
                                            on:click=on_ai_send
                                        >
                                            <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
                                                <path d="M1 7h12M8 2l6 5-6 5"
                                                    stroke="currentColor" stroke-width="1.4"
                                                    stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                        </button>
                                    </div>
                                </div>
                            })}

                        </div>

                    </div>
                })}

            </div> // studio-main

            {move || settings_open.get().then(|| view! {
                <SettingsModal on_close=Callback::new(move |_| settings_open.set(false)) />
            })}
        </div> // studio-shell
    }
}
