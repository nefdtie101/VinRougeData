use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, PrimaryButton, Spinner, StatCard};
use crate::file_analysis::read_file_bytes;
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_column_mapping, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, PbcGroup, ProjectFile};
use super::types::{DataFile, FileSource};
use super::upload::{normalize_map, start_file_upload, upload_file_list};

#[component]
pub fn Step4View(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
    // Hoisted signals so state survives navigation away and back.
    pbc_groups: RwSignal<Vec<PbcGroup>>,
    data_files: RwSignal<Vec<DataFile>>,
    selected_id: RwSignal<Option<String>>,
) -> impl IntoView {
    let active_tab: RwSignal<&'static str> = RwSignal::new("csv");
    let uploading: RwSignal<bool> = RwSignal::new(false);
    let csv_over: RwSignal<bool> = RwSignal::new(false);
    let xlsx_over: RwSignal<bool> = RwSignal::new(false);

    // ── Load PBC groups + pre-existing data files on mount (once only) ────────
    // Guard: skip if already populated to avoid re-running on navigation back.
    spawn_local(async move {
        if pbc_groups.get_untracked().is_empty() {
            if let Ok(groups) = tauri_invoke::<Vec<PbcGroup>>("list_pbc_groups").await {
                pbc_groups.set(groups);
            }
        }
        if data_files.get_untracked().is_empty() {
        if let Ok(files) = tauri_invoke::<Vec<ProjectFile>>("list_project_files").await {
            let mut loaded: Vec<DataFile> = vec![];
            for f in files
                .into_iter()
                .filter(|f| matches!(f.file_type.as_str(), "csv" | "xlsx" | "xls"))
            {
                let cols: Vec<String> = tauri_invoke_args(
                    "get_data_file_headers",
                    serde_json::json!({ "fileId": f.id }),
                )
                .await
                .unwrap_or_default();

                // Try to load previously-saved mappings; only run AI if none exist.
                let mappings = match tauri_invoke_args::<Vec<(String, String)>>(
                    "get_column_mappings",
                    serde_json::json!({ "fileId": f.id }),
                )
                .await
                {
                    Ok(m) if !m.is_empty() => m,
                    _ => {
                        let groups_snap = pbc_groups.get_untracked();
                        let m = match ask_column_mapping(
                            OLLAMA_DEFAULT_URL,
                            OLLAMA_DEFAULT_MODEL,
                            &cols,
                            &groups_snap,
                        )
                        .await
                        {
                            Ok(m) if !m.is_empty() => m,
                            _ => {
                                let all_fields: Vec<String> = groups_snap
                                    .iter()
                                    .flat_map(|g| g.items.iter())
                                    .flat_map(|i| i.fields.iter().cloned())
                                    .collect();
                                normalize_map(&cols, &all_fields)
                            }
                        };
                        // Persist so next load skips AI.
                        let _ = tauri_invoke_args::<()>(
                            "save_column_mappings",
                            serde_json::json!({ "fileId": f.id, "mappings": m }),
                        )
                        .await;
                        m
                    }
                };

                loaded.push(DataFile {
                    local_id: f.name.clone(),
                    name: f.name.clone(),
                    columns: cols,
                    mappings,
                    source: FileSource::Saved(f.id.clone()),
                });
            }
            if !loaded.is_empty() {
                let first_local_id = loaded[0].local_id.clone();
                data_files.set(loaded);
                selected_id.set(Some(first_local_id));
            }
        }
        } // end if data_files.is_empty()
    });

    // Factory for file-input change handlers — called twice in the view (CSV and
    // Excel zones) without ownership issues because all captures are Copy signals.
    let make_file_handler = move || {
        move |ev: web_sys::Event| {
            let input: web_sys::HtmlInputElement = ev.target().unwrap().unchecked_into();
            if let Some(files) = input.files() {
                upload_file_list(files, uploading, status, data_files, pbc_groups, selected_id);
            }
        }
    };

    // Clear upload error when a new file is successfully added
    let upload_err_sig = Signal::derive(move || {
        let s = status.get();
        if s.starts_with("Unsupported") || s.starts_with("Could not") || s.starts_with("Save error")
        {
            Some(s)
        } else {
            None
        }
    });

    let can_proceed = move || !data_files.get().is_empty();

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Header ────────────────────────────────────────────────────────
            <div class="s4-header">
                <div style="display:flex;align-items:center;gap:10px;margin-bottom:3px">
                    <span class="s4-badge">"Step 4"</span>
                    <span class="s4-title">"Data collection"</span>
                </div>
                <div class="s4-subtitle">
                    "Connect to client database or upload data files — data never leaves this machine"
                </div>
            </div>

            // ── Tab row ───────────────────────────────────────────────────────
            <div class="s4-tab-row">
                <button
                    class=move || if active_tab.get() == "sql" { "s4-tab s4-tab--active" } else { "s4-tab" }
                    on:click=move |_| active_tab.set("sql")
                >"SQL connection"</button>
                <button
                    class=move || if active_tab.get() == "csv" { "s4-tab s4-tab--active" } else { "s4-tab" }
                    on:click=move |_| active_tab.set("csv")
                >"CSV / Excel upload"</button>
            </div>

            // ── Pane content ──────────────────────────────────────────────────
            <div style="flex:1;overflow-y:auto;padding:14px">

                // ── SQL stub ──────────────────────────────────────────────────
                {move || (active_tab.get() == "sql").then(|| view! {
                    <div class="s4-sql-stub">
                        <svg width="28" height="28" viewBox="0 0 28 28" fill="none"
                            style="margin:0 auto 10px;display:block;color:var(--w-text-4)">
                            <rect x="4" y="6" width="20" height="16" rx="3"
                                stroke="currentColor" stroke-width="1.3"/>
                            <path d="M4 11h20" stroke="currentColor" stroke-width="1.2"
                                stroke-dasharray="3 2"/>
                            <circle cx="9" cy="17" r="1.5" fill="currentColor"/>
                            <circle cx="14" cy="17" r="1.5" fill="currentColor"/>
                        </svg>
                        "SQL connection — coming soon"
                    </div>
                })}

                // ── CSV / Excel pane ──────────────────────────────────────────
                {move || (active_tab.get() == "csv").then(|| view! {
                    <div>
                        // Drop zones
                        <div class="s4-drop-grid">
                            // CSV zone
                            <label
                                class=move || if csv_over.get() {
                                    "s4-drop-zone s4-drop-zone--over"
                                } else { "s4-drop-zone" }
                                on:dragover=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default(); csv_over.set(true);
                                }
                                on:dragleave=move |_| csv_over.set(false)
                                on:drop=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default(); csv_over.set(false);
                                    if let Some(dt) = ev.data_transfer() {
                                        if let Some(fs) = dt.files() {
                                            upload_file_list(
                                                fs, uploading, status,
                                                data_files, pbc_groups, selected_id,
                                            );
                                        }
                                    }
                                }
                            >
                                <div class="s4-drop-icon">"📄"</div>
                                <div class="s4-drop-label">"CSV file"</div>
                                <div class="s4-drop-hint">"Drag and drop or click to browse"</div>
                                <div class="s4-drop-ext">".csv"</div>
                                <input type="file" accept=".csv" multiple style="display:none"
                                    on:change=make_file_handler() />
                            </label>

                            // Excel zone
                            <label
                                class=move || if xlsx_over.get() {
                                    "s4-drop-zone s4-drop-zone--over"
                                } else { "s4-drop-zone" }
                                on:dragover=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default(); xlsx_over.set(true);
                                }
                                on:dragleave=move |_| xlsx_over.set(false)
                                on:drop=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default(); xlsx_over.set(false);
                                    if let Some(dt) = ev.data_transfer() {
                                        if let Some(fs) = dt.files() {
                                            upload_file_list(
                                                fs, uploading, status,
                                                data_files, pbc_groups, selected_id,
                                            );
                                        }
                                    }
                                }
                            >
                                <div class="s4-drop-icon">"📊"</div>
                                <div class="s4-drop-label">"Excel file"</div>
                                <div class="s4-drop-hint">"Drag and drop or click to browse"</div>
                                <div class="s4-drop-ext">".xlsx · .xls"</div>
                                <input type="file" accept=".xlsx,.xls" multiple style="display:none"
                                    on:change=make_file_handler() />
                            </label>
                        </div>

                        // Upload error banner
                        {move || upload_err_sig.get().map(|msg| view! {
                            <div style="margin-bottom:10px">
                                <Banner message=Signal::derive(move || msg.clone()) variant="error" />
                            </div>
                        })}

                        // Uploading / mapping spinner
                        {move || uploading.get().then(|| {
                            let msg = {
                                let s = status.get();
                                if s.starts_with("Mapping columns") {
                                    s
                                } else {
                                    "Parsing file…".to_string()
                                }
                            };
                            view! {
                                <div class="s4-uploading">
                                    <Spinner size=12 />
                                    {msg}
                                </div>
                            }
                        })}

                        // File list
                        {move || {
                            let files = data_files.get();
                            (!files.is_empty()).then(move || {
                                let items = files.into_iter().map(|d| {
                                    let lid   = d.local_id.clone();
                                    let fname = d.name.clone();
                                    let cols  = d.columns.clone();
                                    let total  = d.columns.len();
                                    let mapped = d.mappings.iter()
                                        .filter(|(_, t)| !t.is_empty()).count();
                                    let (badge_cls, badge_txt) = if total > 0 && mapped == total {
                                        ("s4-file-badge-mapped", "Mapped")
                                    } else if mapped > 0 {
                                        ("s4-file-badge-mapping", "Mapping…")
                                    } else {
                                        ("s4-file-badge-pending", "Pending")
                                    };
                                    let is_csv = fname.to_lowercase().ends_with(".csv");
                                    let file_source = d.source.clone();
                                    view! {
                                        <div
                                            class={
                                                let lid2 = lid.clone();
                                                move || {
                                                    let base = "s4-file-item";
                                                    if selected_id.get().as_deref() == Some(lid2.as_str()) {
                                                        format!("{base} s4-file-item--active")
                                                    } else {
                                                        base.to_string()
                                                    }
                                                }
                                            }
                                            on:click={
                                                let lid = lid.clone();
                                                move |_| selected_id.set(Some(lid.clone()))
                                            }
                                        >
                                            <span class="s4-file-icon">
                                                {if is_csv { "📄" } else { "📊" }}
                                            </span>
                                            <div class="s4-file-info">
                                                <div class="s4-file-name">{fname.clone()}</div>
                                                <div class="s4-file-meta">
                                                    {format!("{mapped} / {total} columns mapped")}
                                                </div>
                                            </div>
                                            <span class=badge_cls>{badge_txt}</span>
                                            <button
                                                class="s4-file-delete"
                                                title="Remove file"
                                                on:click={
                                                    let lid = lid.clone();
                                                    move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        let lid = lid.clone();
                                                        match &file_source {
                                                            FileSource::Saved(id) => {
                                                                let id = id.clone();
                                                                spawn_local(async move {
                                                                    let _ = tauri_invoke_args::<()>(
                                                                        "delete_project_file",
                                                                        serde_json::json!({ "fileId": id }),
                                                                    ).await;
                                                                    data_files.update(|v| v.retain(|d| d.local_id != lid));
                                                                    selected_id.update(|s| {
                                                                        if s.as_deref() == Some(lid.as_str()) {
                                                                            *s = None;
                                                                        }
                                                                    });
                                                                });
                                                            }
                                                            FileSource::Browser(_) => {
                                                                data_files.update(|v| v.retain(|d| d.local_id != lid));
                                                                selected_id.update(|s| {
                                                                    if s.as_deref() == Some(lid.as_str()) {
                                                                        *s = None;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    }
                                                }
                                            >
                                                "✕"
                                            </button>
                                        </div>
                                    }
                                }).collect_view();
                                view! { <div class="s4-file-list">{items}</div> }
                            })
                        }}

                        // Column mapping panel (shown when a file is selected)
                        {move || {
                            let sel_id = selected_id.get()?;
                            let files  = data_files.get();
                            let idx    = files.iter().position(|d| d.local_id == sel_id)?;
                            let df     = files.get(idx)?;
                            if df.columns.is_empty() { return None; }

                            let mut pbc_fields: Vec<String> = pbc_groups.get()
                                .iter()
                                .flat_map(|g| g.items.iter())
                                .flat_map(|i| i.fields.iter().cloned())
                                .collect();
                            pbc_fields.sort();
                            pbc_fields.dedup();

                            let fname   = df.name.clone();
                            let cols    = df.columns.clone();
                            let maps    = df.mappings.clone();
                            let file_id = match &df.source {
                                FileSource::Saved(id) => Some(id.clone()),
                                FileSource::Browser(_) => None,
                            };

                            Some(view! {
                                <div class="s4-col-map-panel">
                                    <div class="s4-col-map-head">
                                        "Column mapping — " {fname}
                                    </div>
                                    <div class="s4-col-map-grid-header">
                                        <span>"Source column"</span>
                                        <span>"PBC field"</span>
                                    </div>
                                    {cols.into_iter().enumerate().map(|(i, col)| {
                                        let current    = maps.get(i)
                                            .map(|(_, t)| t.clone())
                                            .unwrap_or_default();
                                        let pbc_fields = pbc_fields.clone();
                                        view! {
                                            <div class="s4-col-map-row">
                                                <span class="s4-col-map-src">{col}</span>
                                                <select
                                                    class="s4-col-map-select"
                                                    prop:value=current.clone()
                                                    on:change={
                                                        let file_id = file_id.clone();
                                                        move |ev| {
                                                            let val = event_target_value(&ev);
                                                            data_files.update(|v| {
                                                                if let Some(d) = v.get_mut(idx) {
                                                                    if let Some(m) = d.mappings.get_mut(i) {
                                                                        m.1 = val;
                                                                    }
                                                                }
                                                            });
                                                            // Persist the updated mappings immediately.
                                                            if let Some(fid) = file_id.clone() {
                                                                let mappings = data_files
                                                                    .get_untracked()
                                                                    .get(idx)
                                                                    .map(|d| d.mappings.clone())
                                                                    .unwrap_or_default();
                                                                spawn_local(async move {
                                                                    let _ = tauri_invoke_args::<()>(
                                                                        "save_column_mappings",
                                                                        serde_json::json!({
                                                                            "fileId": fid,
                                                                            "mappings": mappings
                                                                        }),
                                                                    )
                                                                    .await;
                                                                });
                                                            }
                                                        }
                                                    }
                                                >
                                                    <option value="">"— not mapped —"</option>
                                                    {pbc_fields.iter().map(|f| {
                                                        let f_val  = f.clone();
                                                        let f_disp = f_val.clone();
                                                        let sel    = f_val == current;
                                                        view! {
                                                            <option value=f_val selected=sel>
                                                                {f_disp}
                                                            </option>
                                                        }
                                                    }).collect_view()}
                                                </select>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            })
                        }}
                    </div>
                })}
            </div>

            // ── Stat cards + status bar ───────────────────────────────────────
            {move || {
                let files = data_files.get();
                (!files.is_empty()).then(move || {
                    let n_files   = files.len();
                    let n_mapped  = files.iter()
                        .filter(|d| !d.mappings.is_empty() &&
                            d.mappings.iter().all(|(_, t)| !t.is_empty()))
                        .count();
                    let n_cols    = files.iter().map(|d| d.columns.len()).sum::<usize>();
                    view! {
                        <div style="flex-shrink:0;display:flex;gap:8px;padding:8px 14px;\
                                    border-top:0.5px solid var(--w-border)">
                            <StatCard label="Files"
                                value=Signal::derive(move || n_files.to_string()) />
                            <StatCard label="Fully mapped"
                                value=Signal::derive(move || n_mapped.to_string())
                                green=true />
                            <StatCard label="Total columns"
                                value=Signal::derive(move || n_cols.to_string()) />
                        </div>
                    }
                })
            }}

            <div class="s4-status-bar">
                <span class=move || {
                    if data_files.get().is_empty() { "s4-dot s4-dot--idle" }
                    else { "s4-dot s4-dot--ready" }
                }></span>
                <span class="s4-status-text">
                    {move || {
                        let files = data_files.get();
                        if files.is_empty() {
                            "No data files uploaded".to_string()
                        } else {
                            let n = files.len();
                            format!("{n} file{} uploaded", if n == 1 { "" } else { "s" })
                        }
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(3)) />
                    <PrimaryButton
                        label="Proceed to 4a"
                        disabled=Signal::derive(move || !can_proceed() || uploading.get())
                        on_click=Callback::new(move |()| {
                            if !can_proceed() { return; }
                            let files = data_files.get_untracked();
                            spawn_local(async move {
                                for df in files {
                                    if df.columns.is_empty() { continue; }
                                    let mapped = df.mappings.clone();

                                    // For browser files: save bytes to project first.
                                    // For already-saved files: use their existing ID.
                                    let file_id = match df.source {
                                        FileSource::Browser(browser_file) => {
                                            let bytes = match read_file_bytes(&browser_file).await {
                                                Ok(b) => b,
                                                Err(_) => continue,
                                            };
                                            let pf: ProjectFile = match tauri_invoke_args(
                                                "add_data_file",
                                                serde_json::json!({
                                                    "name": df.name,
                                                    "bytes": bytes
                                                }),
                                            )
                                            .await
                                            {
                                                Ok(f) => f,
                                                Err(_) => continue,
                                            };
                                            pf.id
                                        }
                                        FileSource::Saved(id) => id,
                                    };

                                    let _ = tauri_invoke_args::<String>(
                                        "import_data_file",
                                        serde_json::json!({
                                            "fileId": file_id,
                                            "mappings": mapped,
                                            "sheet": null
                                        }),
                                    )
                                    .await;
                                }
                                audit_ui_step.set(5);
                            });
                        })
                    />
                </div>
            </div>

        </div>
    }
}
