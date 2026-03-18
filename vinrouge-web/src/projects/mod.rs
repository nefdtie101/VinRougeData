use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::types::{AiMessage, AuditProcessWithControls, Project, ProjectFile};
use crate::storage::{AuditSetupState, ls_get, ls_set};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, ask_ollama_json, ask_ollama_wasm};
use crate::step1;
use crate::step2;
use crate::step3;
use crate::step4;

// ── Projects enums ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum RightPanel { Empty, CreateWizard, ActiveProject, CreateSuccess }

// ── ProjectsView ──────────────────────────────────────────────────────────────

#[component]
pub fn ProjectsView() -> impl IntoView {
    // ── Core state ────────────────────────────────────────────────────────────
    let projects: RwSignal<Vec<Project>>          = RwSignal::new(vec![]);
    let active_project: RwSignal<Option<Project>> = RwSignal::new(None);
    let project_files: RwSignal<Vec<ProjectFile>> = RwSignal::new(vec![]);
    let ai_messages: RwSignal<Vec<AiMessage>>     = RwSignal::new(vec![]);
    let right_panel: RwSignal<RightPanel>         = RwSignal::new(RightPanel::Empty);
    let status: RwSignal<String>                  = RwSignal::new(String::new());
    let last_created: RwSignal<Option<Project>>   = RwSignal::new(None);
    let search: RwSignal<String>                  = RwSignal::new(String::new());

    // ── Wizard state (3 steps) ────────────────────────────────────────────────
    let wiz_step: RwSignal<u8>                    = RwSignal::new(1);
    // Step 1
    let wiz_name: RwSignal<String>                = RwSignal::new(String::new());
    let wiz_client: RwSignal<String>              = RwSignal::new(String::new());
    let wiz_ref_: RwSignal<String>                = RwSignal::new(String::new());
    let wiz_start: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_end: RwSignal<String>                 = RwSignal::new(String::new());
    let wiz_due: RwSignal<String>                 = RwSignal::new(String::new());
    let wiz_type: RwSignal<String>                = RwSignal::new("Compliance".to_string());
    let wiz_notes: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_save_dir: RwSignal<Option<String>>    = RwSignal::new(None);
    // Step 2
    let wiz_standards: RwSignal<Vec<String>>      = RwSignal::new(vec![]);
    let wiz_scope: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_materiality: RwSignal<String>         = RwSignal::new(String::new());
    let wiz_risk_fw: RwSignal<String>             = RwSignal::new("High / Medium / Low".to_string());

    // ── Audit plan state ──────────────────────────────────────────────────────
    let audit_plan: RwSignal<Vec<AuditProcessWithControls>> = RwSignal::new(vec![]);
    let sop_analyzing: RwSignal<Option<String>>             = RwSignal::new(None); // file_id

    // ── Sidebar resize ────────────────────────────────────────────────────────
    let sidebar_width: RwSignal<f64> = RwSignal::new(260.0);
    let is_dragging:   RwSignal<bool> = RwSignal::new(false);

    // ── Chat state ────────────────────────────────────────────────────────────
    let chat_input: RwSignal<String>  = RwSignal::new(String::new());
    let chat_loading: RwSignal<bool>  = RwSignal::new(false);

    // ── Audit setup state (step 1) ────────────────────────────────────────────
    let setup_standards: RwSignal<Vec<(String, bool)>> = RwSignal::new(vec![
        ("ISO 27001".to_string(), true),
        ("SOC 2".to_string(),     true),
        ("GDPR".to_string(),      false),
        ("GAAP".to_string(),      false),
        ("IFRS".to_string(),      false),
        ("PCI-DSS".to_string(),   false),
    ]);
    let setup_scope: RwSignal<Vec<String>>         = RwSignal::new(vec![]);
    let setup_approved: RwSignal<bool>             = RwSignal::new(false);
    let setup_new_std: RwSignal<String>            = RwSignal::new(String::new());
    let setup_new_scope: RwSignal<String>          = RwSignal::new(String::new());
    let ai_setup_prompt: RwSignal<String>          = RwSignal::new(String::new());
    let ai_setup_loading: RwSignal<bool>           = RwSignal::new(false);
    let ai_setup_summary: RwSignal<Option<String>> = RwSignal::new(None);
    let ai_setup_err: RwSignal<bool>               = RwSignal::new(false);
    let audit_ui_step: RwSignal<u8>                = RwSignal::new(1);
    let plan_needs_regen: RwSignal<bool>           = RwSignal::new(false);
    let sop_extracting: RwSignal<bool>             = RwSignal::new(false);

    // Persist audit setup to localStorage whenever any part changes
    Effect::new(move || {
        let stds     = setup_standards.get();
        let scope    = setup_scope.get();
        let approved = setup_approved.get();
        let step     = audit_ui_step.get();
        if let Some(p) = active_project.get() {
            let state = AuditSetupState { step, standards: stds, scope, approved };
            if let Ok(json) = serde_json::to_string(&state) {
                ls_set(&format!("audit_setup_v1_{}", p.id), &json);
            }
        }
    });

    // ── Load projects on mount ────────────────────────────────────────────────
    spawn_local(async move {
        match tauri_invoke::<Vec<Project>>("list_projects").await {
            Ok(list) => projects.set(list),
            Err(e)   => status.set(format!("Failed to load projects: {e}")),
        }
    });

    let refresh_project_data = move || {
        spawn_local(async move {
            if let Ok(f) = tauri_invoke::<Vec<ProjectFile>>("list_project_files").await { project_files.set(f); }
            if let Ok(m) = tauri_invoke::<Vec<AiMessage>>("list_ai_messages").await      { ai_messages.set(m); }
            if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await { audit_plan.set(p); }
        });
    };

    // ── Reset wizard and open it ──────────────────────────────────────────────
    let open_wiz = move || {
        wiz_step.set(1);
        wiz_name.set(String::new());
        wiz_client.set(String::new());
        wiz_ref_.set(String::new());
        wiz_start.set(String::new());
        wiz_end.set(String::new());
        wiz_due.set(String::new());
        wiz_type.set("Compliance".to_string());
        wiz_notes.set(String::new());
        wiz_save_dir.set(None);
        wiz_standards.set(vec![]);
        wiz_scope.set(String::new());
        wiz_materiality.set(String::new());
        wiz_risk_fw.set("High / Medium / Low".to_string());
        right_panel.set(RightPanel::CreateWizard);
    };

    // ── Open a project ────────────────────────────────────────────────────────
    let open_project = move |path: String| {
        spawn_local(async move {
            match tauri_invoke_args::<Project>(
                "open_project",
                serde_json::json!({ "path": path }),
            ).await {
                Ok(p) => {
                    // Restore persisted audit setup for this project
                    if let Some(json) = ls_get(&format!("audit_setup_v1_{}", p.id)) {
                        if let Ok(s) = serde_json::from_str::<AuditSetupState>(&json) {
                            setup_standards.set(s.standards);
                            setup_scope.set(s.scope);
                            setup_approved.set(s.approved);
                            audit_ui_step.set(s.step);
                        }
                    } else {
                        // Fresh project — reset to defaults
                        setup_standards.set(vec![
                            ("ISO 27001".to_string(), true),
                            ("SOC 2".to_string(),     true),
                            ("GDPR".to_string(),      false),
                            ("GAAP".to_string(),      false),
                            ("IFRS".to_string(),      false),
                            ("PCI-DSS".to_string(),   false),
                        ]);
                        setup_scope.set(vec![]);
                        setup_approved.set(false);
                        audit_ui_step.set(1);
                    }
                    plan_needs_regen.set(false);
                    active_project.set(Some(p));
                    right_panel.set(RightPanel::ActiveProject);
                    refresh_project_data();
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Pick save folder ──────────────────────────────────────────────────────
    let on_pick_folder = move |_| {
        spawn_local(async move {
            match tauri_invoke::<Option<String>>("pick_project_folder").await {
                Ok(Some(p)) => wiz_save_dir.set(Some(p)),
                Ok(None)    => {}
                Err(e)      => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Create project (step 3 confirm) ──────────────────────────────────────
    let on_create = move |_| {
        let name = wiz_name.get();
        if name.trim().is_empty() { return; }
        let args = serde_json::json!({
            "name":           name,
            "saveDir":        wiz_save_dir.get(),
            "client":         wiz_client.get(),
            "engagementRef":  wiz_ref_.get(),
            "periodStart":    wiz_start.get(),
            "periodEnd":      wiz_end.get(),
            "reportDue":      wiz_due.get(),
            "auditType":      wiz_type.get(),
            "notes":          wiz_notes.get(),
            "standards":      wiz_standards.get(),
            "scope":          wiz_scope.get(),
            "materiality":    wiz_materiality.get(),
            "riskFramework":  wiz_risk_fw.get(),
        });
        spawn_local(async move {
            match tauri_invoke_args::<Project>("create_project", args).await {
                Ok(p) => {
                    last_created.set(Some(p));
                    if let Ok(list) = tauri_invoke::<Vec<Project>>("list_projects").await {
                        projects.set(list);
                    }
                    right_panel.set(RightPanel::CreateSuccess);
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Add file — uploads, then extracts setup basics from SOP via Ollama ─────
    let on_add_file = move |_: web_sys::MouseEvent| {
        spawn_local(async move {
            let f = match tauri_invoke::<Option<ProjectFile>>("pick_and_add_file").await {
                Ok(Some(f)) => f,
                Ok(None)    => return,
                Err(e)      => { status.set(format!("Error: {e}")); return; }
            };

            let is_sop = f.file_type == "txt" || f.file_type == "pdf";
            refresh_project_data();

            if !is_sop {
                status.set(format!("Added \"{}\"", f.name));
                return;
            }

            // Read SOP text
            let file_id   = f.id.clone();
            let file_name = f.name.clone();
            sop_extracting.set(true);
            status.set(format!("Reading \"{}\"...", file_name));

            let text = match tauri_invoke_args::<String>(
                "read_project_file",
                serde_json::json!({ "fileId": file_id.clone() }),
            ).await {
                Ok(t) => t,
                Err(e) => {
                    status.set(format!("Could not read file: {e}"));
                    sop_extracting.set(false);
                    return;
                }
            };

            // Run EXTRACT_SETUP to auto-populate standards + scope
            status.set(format!("Extracting setup from \"{}\"...", file_name));
            let extract_prompt = format!("{}\n\n{}", crate::step1::prompts::EXTRACT_SETUP, text);
            match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &extract_prompt).await {
                Ok(raw) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                        // Merge suggested standards (preserve existing selections, add new ones)
                        if let Some(arr) = v["standards"].as_array() {
                            setup_standards.update(|current| {
                                for n in arr {
                                    if let Some(s) = n.as_str() {
                                        let s = s.to_string();
                                        if !current.iter().any(|(x, _)| x.to_lowercase() == s.to_lowercase()) {
                                            current.push((s, true));
                                        } else if let Some(e) = current.iter_mut().find(|(x, _)| x.to_lowercase() == s.to_lowercase()) {
                                            e.1 = true; // select it if suggested
                                        }
                                    }
                                }
                            });
                        }
                        // Replace scope with extracted processes
                        if let Some(arr) = v["scope"].as_array() {
                            let new_scope: Vec<String> = arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect();
                            if !new_scope.is_empty() {
                                setup_scope.set(new_scope);
                            }
                        }
                    }
                }
                Err(_) => {} // silent — user can configure manually
            }

            // If plan already exists, mark it as needing regeneration
            if !audit_plan.get().is_empty() {
                plan_needs_regen.set(true);
            }

            sop_extracting.set(false);
            status.set(format!("\"{}\" loaded — review setup then generate plan", file_name));
        });
    };

    // ── Chat send ─────────────────────────────────────────────────────────────
    let on_chat_send = move |_: web_sys::MouseEvent| {
        let q = chat_input.get();
        if q.trim().is_empty() || chat_loading.get() { return; }
        chat_loading.set(true);
        spawn_local(async move {
            let _ = tauri_invoke_args::<AiMessage>(
                "save_ai_message",
                serde_json::json!({ "role": "user", "content": q.clone() }),
            ).await;
            let reply = match ask_ollama_wasm(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, "", &q).await {
                Ok(r)  => r,
                Err(e) => format!("Error: {e}"),
            };
            let _ = tauri_invoke_args::<AiMessage>(
                "save_ai_message",
                serde_json::json!({ "role": "assistant", "content": reply }),
            ).await;
            chat_input.set(String::new());
            if let Ok(msgs) = tauri_invoke::<Vec<AiMessage>>("list_ai_messages").await {
                ai_messages.set(msgs);
            }
            chat_loading.set(false);
        });
    };

    // ── Validation ────────────────────────────────────────────────────────────
    let step1_valid = move || {
        !wiz_name.get().trim().is_empty()
            && !wiz_client.get().trim().is_empty()
            && !wiz_start.get().is_empty()
            && !wiz_end.get().is_empty()
    };

    let step2_valid = move || !wiz_standards.get().is_empty();

    // ── View ──────────────────────────────────────────────────────────────────
    view! {
        <div
            class=move || if is_dragging.get() { "projects-shell projects-shell--dragging" } else { "projects-shell" }
            on:mousemove=move |ev| {
                if is_dragging.get() {
                    let x = ev.client_x() as f64;
                    sidebar_width.set(x.max(160.0).min(520.0));
                }
            }
            on:mouseup=move   |_| is_dragging.set(false)
            on:mouseleave=move |_| is_dragging.set(false)
        >

            // ── Left sidebar ─────────────────────────────────────────────────
            <div
                class="proj-sidebar"
                style=move || format!("width:{}px;min-width:{}px", sidebar_width.get() as u32, sidebar_width.get() as u32)
            >
                <div class="proj-sidebar-header">
                    <span class="proj-sidebar-title">"PROJECTS"</span>
                    <button class="proj-add-btn" title="New project" on:click=move |_| open_wiz()>
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                            <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                        </svg>
                    </button>
                </div>

                <div class="proj-search-box">
                    <svg class="proj-search-icon" width="13" height="13" viewBox="0 0 13 13" fill="none">
                        <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.2"/>
                        <path d="M9 9l2.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                    </svg>
                    <input
                        class="proj-search-input"
                        type="text"
                        placeholder="Search projects..."
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
                                let path      = p.path.clone();
                                let path_del  = p.path.clone();
                                let name      = p.name.clone();
                                let date      = p.created_at.get(..10).unwrap_or("").to_string();
                                let p_id      = p.id.clone();
                                let p_id_del  = p.id.clone();
                                view! {
                                    <div
                                        class=move || {
                                            let active = active_project.get()
                                                .as_ref().map(|a| a.id == p_id).unwrap_or(false);
                                            if active { "proj-list-item proj-list-item--active" }
                                            else      { "proj-list-item" }
                                        }
                                        on:click=move |_| open_project(path.clone())
                                    >
                                        <div class="proj-item-row1">
                                            <span class="proj-item-name">{name}</span>
                                            <button
                                                class="proj-delete-btn"
                                                title="Delete project"
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    let path_d = path_del.clone();
                                                    let pid    = p_id_del.clone();
                                                    spawn_local(async move {
                                                        let args = serde_json::json!({ "path": path_d });
                                                        match tauri_invoke_args::<()>("delete_project", args).await {
                                                            Ok(_) => {
                                                                if active_project.get().as_ref().map(|a| a.id == pid).unwrap_or(false) {
                                                                    active_project.set(None);
                                                                    right_panel.set(RightPanel::Empty);
                                                                }
                                                                if let Ok(list) = tauri_invoke::<Vec<Project>>("list_projects").await {
                                                                    projects.set(list);
                                                                }
                                                            }
                                                            Err(e) => status.set(format!("Delete failed: {e}")),
                                                        }
                                                    });
                                                }
                                            >
                                                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                                    <path d="M2 3h8M5 3V2h2v1M10 3l-.8 7H2.8L2 3"
                                                        stroke="currentColor" stroke-width="1.2"
                                                        stroke-linecap="round" stroke-linejoin="round"/>
                                                </svg>
                                            </button>
                                        </div>
                                        <div class="proj-item-meta">{date}</div>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>

                <button class="proj-new-btn-dashed" on:click=move |_| open_wiz()>
                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                        <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                    "New project"
                </button>
            </div>

            // ── Resize handle ─────────────────────────────────────────────────
            <div
                class=move || if is_dragging.get() { "proj-resize-handle proj-resize-handle--active" } else { "proj-resize-handle" }
                on:mousedown=move |ev| { ev.prevent_default(); is_dragging.set(true); }
            />

            // ── Right panel ───────────────────────────────────────────────────
            <div class="proj-main">

                // Status toast
                {move || {
                    let s = status.get();
                    (!s.is_empty()).then(|| view! { <div class="proj-status-toast">{s}</div> })
                }}

                // ── Empty state ───────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::Empty).then(|| view! {
                    <div class="proj-empty-state">
                        <div class="proj-empty-icon">
                            <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
                                <path d="M13 3L4 7.5v11L13 23l9-4.5v-11L13 3z"
                                    stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                                <path d="M13 3v20M4 7.5l9 5 9-5" stroke="currentColor" stroke-width="1.3"/>
                            </svg>
                        </div>
                        <div class="proj-empty-title">"No project selected"</div>
                        <div class="proj-empty-sub">
                            "Select an existing project from the sidebar, or create a new one to get started."
                        </div>
                        <button class="wiz-btn-primary" on:click=move |_| open_wiz()>
                            "Create new project"
                        </button>
                    </div>
                })}

                // ── Create wizard ─────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::CreateWizard).then(|| view! {
                    <div class="wiz-container">

                        // Step bar (3 steps)
                        <div class="wiz-step-bar">
                            {move || {
                                let cur = wiz_step.get();
                                let steps: &[(&str, &str)] = &[
                                    ("1", "Project details"),
                                    ("2", "Standards & scope"),
                                    ("3", "Review & create"),
                                ];
                                steps.iter().enumerate().map(|(i, &(num, label))| {
                                    let idx = (i + 1) as u8;
                                    let cls = if cur == idx      { "wiz-step wiz-step--active" }
                                              else if cur > idx  { "wiz-step wiz-step--done"   }
                                              else               { "wiz-step"                   };
                                    view! {
                                        <>
                                            {(i > 0).then(|| view! { <div class="wiz-step-line"></div> })}
                                            <div class=cls>
                                                <div class="wiz-step-num">{num}</div>
                                                <span class="wiz-step-label">{label}</span>
                                            </div>
                                        </>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        // Step panels
                        {move || {
                            let step = wiz_step.get();

                            // ── Step 1: Project details ───────────────────────
                            if step == 1 { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Project details"</div>
                                        <div class="wiz-sub">"Basic information about this audit engagement"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div>
                                            <div class="wiz-section-label">"ENGAGEMENT"</div>
                                            <div style="display:flex;flex-direction:column;gap:12px">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Project name "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="text"
                                                        placeholder="e.g. Acme Corp — ISO 27001 Audit 2026"
                                                        prop:value=move || wiz_name.get()
                                                        on:input=move |ev| wiz_name.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-two-col">
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">
                                                            "Client / entity name "
                                                            <span class="wiz-req">"*"</span>
                                                        </label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="Client name"
                                                            prop:value=move || wiz_client.get()
                                                            on:input=move |ev| wiz_client.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Engagement reference"</label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="e.g. ENG-2026-041"
                                                            prop:value=move || wiz_ref_.get()
                                                            on:input=move |ev| wiz_ref_.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"AUDIT PERIOD"</div>
                                            <div class="wiz-three-col">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Period start "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_start.get()
                                                        on:input=move |ev| wiz_start.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Period end "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_end.get()
                                                        on:input=move |ev| wiz_end.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-field">
                                                    <label class="wiz-label">"Report due date"</label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_due.get()
                                                        on:input=move |ev| wiz_due.set(event_target_value(&ev))
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"AUDIT TYPE"</div>
                                            <div class="wiz-audit-cards">
                                                {["Compliance", "Financial", "Operational"].into_iter().map(|t| view! {
                                                    <div
                                                        class=move || if wiz_type.get() == t {
                                                            "wiz-audit-card wiz-audit-card--selected"
                                                        } else { "wiz-audit-card" }
                                                        on:click=move |_| wiz_type.set(t.to_string())
                                                    >
                                                        <div class="wiz-audit-name">{t}</div>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"NOTES"</div>
                                            <textarea class="wiz-textarea"
                                                placeholder="Optional — scope limitations, special instructions..."
                                                prop:value=move || wiz_notes.get()
                                                on:input=move |ev| wiz_notes.set(event_target_value(&ev))
                                            ></textarea>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"SAVE LOCATION"</div>
                                            <div class="wiz-folder-row">
                                                <span class="wiz-folder-path">
                                                    {move || wiz_save_dir.get()
                                                        .unwrap_or_else(|| "~/VinRouge/projects (default)".to_string())}
                                                </span>
                                                <button class="wiz-btn-ghost" on:click=on_pick_folder>
                                                    "Browse…"
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            {move || if step1_valid() {
                                                "Ready to continue"
                                            } else {
                                                "Fill in required fields to continue"
                                            }}
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| right_panel.set(RightPanel::Empty)>
                                                "Cancel"
                                            </button>
                                            <button class="wiz-btn-primary"
                                                disabled=move || !step1_valid()
                                                on:click=move |_| wiz_step.set(2)>
                                                "Next — Standards"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()

                            // ── Step 2: Standards & scope ─────────────────────
                            } else if step == 2 { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Standards & scope"</div>
                                        <div class="wiz-sub">"Select the applicable standards and define the audit scope"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div>
                                            <div class="wiz-section-label">"APPLICABLE STANDARDS"</div>
                                            <div class="wiz-standards-grid">
                                                {["ISO 27001", "SOC 2", "GDPR", "GAAP", "ISA 315", "SOX"]
                                                    .into_iter().map(|s| view! {
                                                    <div
                                                        class=move || if wiz_standards.get().contains(&s.to_string()) {
                                                            "wiz-standard-item wiz-standard-item--selected"
                                                        } else { "wiz-standard-item" }
                                                        on:click=move |_| {
                                                            wiz_standards.update(|v| {
                                                                let name = s.to_string();
                                                                if v.contains(&name) { v.retain(|x| x != &name); }
                                                                else { v.push(name); }
                                                            });
                                                        }
                                                    >
                                                        <div class=move || if wiz_standards.get().contains(&s.to_string()) {
                                                            "wiz-std-chk wiz-std-chk--checked"
                                                        } else { "wiz-std-chk" }>
                                                            {move || wiz_standards.get().contains(&s.to_string()).then(|| view! {
                                                                <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
                                                                    <path d="M1 4l3 3 5-6" stroke="white"
                                                                        stroke-width="1.5" stroke-linecap="round"
                                                                        stroke-linejoin="round"/>
                                                                </svg>
                                                            })}
                                                        </div>
                                                        <div class="wiz-std-name">{s}</div>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"SCOPE STATEMENT"</div>
                                            <div style="display:flex;flex-direction:column;gap:12px">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">"In-scope systems & processes"</label>
                                                    <textarea class="wiz-textarea"
                                                        placeholder="Describe the systems, processes, or business units within scope..."
                                                        prop:value=move || wiz_scope.get()
                                                        on:input=move |ev| wiz_scope.set(event_target_value(&ev))
                                                    ></textarea>
                                                </div>
                                                <div class="wiz-two-col">
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Materiality threshold"</label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="e.g. R50,000 / 5% of revenue"
                                                            prop:value=move || wiz_materiality.get()
                                                            on:input=move |ev| wiz_materiality.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Risk rating framework"</label>
                                                        <select class="wiz-input"
                                                            prop:value=move || wiz_risk_fw.get()
                                                            on:change=move |ev| wiz_risk_fw.set(event_target_value(&ev))
                                                        >
                                                            <option value="High / Medium / Low">"High / Medium / Low"</option>
                                                            <option value="1–5 numeric scale">"1–5 numeric scale"</option>
                                                            <option value="Red / Amber / Green">"Red / Amber / Green"</option>
                                                            <option value="Custom">"Custom"</option>
                                                        </select>
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            {move || {
                                                let stds = wiz_standards.get();
                                                if stds.is_empty() {
                                                    "Select at least one standard".to_string()
                                                } else {
                                                    format!("{} selected", stds.join(", "))
                                                }
                                            }}
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| wiz_step.set(1)>"Back"</button>
                                            <button class="wiz-btn-primary"
                                                disabled=move || !step2_valid()
                                                on:click=move |_| wiz_step.set(3)>
                                                "Review & create"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()

                            // ── Step 3: Review & create ───────────────────────
                            } else { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Review & create"</div>
                                        <div class="wiz-sub">"Confirm all details before creating the project"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div class="wiz-review-alert">
                                            <svg width="15" height="15" viewBox="0 0 15 15" fill="none"
                                                style="flex-shrink:0;margin-top:1px">
                                                <circle cx="7.5" cy="7.5" r="6.5"
                                                    stroke="currentColor" stroke-width="1.1"/>
                                                <path d="M7.5 5v3.5M7.5 10.5h.01"
                                                    stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
                                            </svg>
                                            "A project record and empty audit trail log will be created on-device. No data leaves this machine."
                                        </div>

                                        // Project details card
                                        <div class="wiz-review-card">
                                            <div class="wiz-review-card-head">
                                                "Project details"
                                                <button class="wiz-review-edit"
                                                    on:click=move |_| wiz_step.set(1)>"Edit"</button>
                                            </div>
                                            <div class="wiz-review-grid">
                                                <div>
                                                    <div class="wiz-rv-label">"Project name"</div>
                                                    <div class="wiz-rv-val">{move || wiz_name.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Client"</div>
                                                    <div class="wiz-rv-val">{move || wiz_client.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Period"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || format!("{} → {}", wiz_start.get(), wiz_end.get())}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Audit type"</div>
                                                    <div class="wiz-rv-val">{move || wiz_type.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Reference"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let r = wiz_ref_.get();
                                                            if r.is_empty() { "—".to_string() } else { r }
                                                        }}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Report due"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let d = wiz_due.get();
                                                            if d.is_empty() { "—".to_string() } else { d }
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                        </div>

                                        // Standards & scope card
                                        <div class="wiz-review-card">
                                            <div class="wiz-review-card-head">
                                                "Standards & scope"
                                                <button class="wiz-review-edit"
                                                    on:click=move |_| wiz_step.set(2)>"Edit"</button>
                                            </div>
                                            <div class="wiz-review-tags">
                                                {move || wiz_standards.get().into_iter()
                                                    .map(|s| view! { <span class="wiz-std-tag">{s}</span> })
                                                    .collect_view()}
                                            </div>
                                            <div class="wiz-review-grid" style="padding-top:0">
                                                <div>
                                                    <div class="wiz-rv-label">"Materiality threshold"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let m = wiz_materiality.get();
                                                            if m.is_empty() { "—".to_string() } else { m }
                                                        }}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Risk rating framework"</div>
                                                    <div class="wiz-rv-val">{move || wiz_risk_fw.get()}</div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            "Project will be saved locally — no data leaves this machine"
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| wiz_step.set(2)>"Back"</button>
                                            <button class="wiz-btn-primary" on:click=on_create>
                                                "Create project"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any() }
                        }}

                    </div> // wiz-container
                })}

                // ── Success screen ────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::CreateSuccess).then(|| {
                    let pname = last_created.get().as_ref().map(|p| p.name.clone()).unwrap_or_default();
                    let stds  = wiz_standards.get().join(", ");
                    let stds2 = if stds.is_empty() { "—".to_string() } else { stds };
                    view! {
                        <div class="proj-success">
                            <div class="proj-success-icon">
                                <svg width="26" height="26" viewBox="0 0 28 28" fill="none">
                                    <path d="M5 14l7 7 11-11" stroke="currentColor"
                                        stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                            </div>
                            <div class="proj-success-title">"Project created"</div>
                            <div class="proj-success-sub">
                                "Your project has been created and the audit trail log initialised."
                            </div>
                            <div class="proj-success-meta">
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Client"</div>
                                    <div class="proj-smi-val">{move || wiz_client.get()}</div>
                                </div>
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Standards"</div>
                                    <div class="proj-smi-val">{stds2}</div>
                                </div>
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Status"</div>
                                    <div class="proj-smi-val proj-smi-val--accent">"Draft"</div>
                                </div>
                            </div>
                            <div style="display:flex;gap:10px">
                                <button class="wiz-btn-ghost"
                                    on:click=move |_| right_panel.set(RightPanel::Empty)>
                                    "Back to projects"
                                </button>
                                <button class="wiz-btn-primary"
                                    on:click=move |_| {
                                        if let Some(p) = last_created.get() {
                                            open_project(p.path.clone());
                                        }
                                    }>
                                    "Open project"
                                </button>
                            </div>
                        </div>
                    }
                })}

                // ── Active project panel ──────────────────────────────────────
                {move || (right_panel.get() == RightPanel::ActiveProject).then(|| {
                    let pname = active_project.get().as_ref().map(|p| p.name.clone()).unwrap_or_default();
                    view! {
                        <div class="proj-active-panel" style="display:flex;flex-direction:column;overflow:hidden">

                            // ── Step trail ────────────────────────────────────
                            <div class="audit-top-bar">
                                <div class="audit-step-trail">
                                    <div class="audit-step-crumb" style="cursor:pointer" on:click=move |_| { audit_ui_step.set(1); }>
                                        <div class=move || { let s = audit_ui_step.get(); if s != 1 { "audit-step-num done" } else { "audit-step-num active" } }>
                                            {move || if audit_ui_step.get() != 1 {
                                                view! { <svg width="8" height="8" viewBox="0 0 10 10" fill="none"><polyline points="1.5,5 4,7.5 8.5,2.5" stroke="#fff" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                                            } else { view! { "1" }.into_any() }}
                                        </div>
                                        <span class=move || if audit_ui_step.get() == 1 { "audit-step-label active" } else { "audit-step-label muted" }>{pname.clone()}</span>
                                    </div>
                                    <span class="audit-step-sep">"›"</span>
                                    <div class="audit-step-crumb">
                                        <div class=move || if audit_ui_step.get() == 2 { "audit-step-num active" } else { "audit-step-num pending" }>"2"</div>
                                        <span class=move || if audit_ui_step.get() == 2 { "audit-step-label active" } else { "audit-step-label muted" }>"Audit plan"</span>
                                    </div>
                                    <span class="audit-step-sep">"›"</span>
                                    <div class="audit-step-crumb"
                                        style=move || { let s = audit_ui_step.get(); if s == 3 || s == 4 || s == 5 { "cursor:pointer" } else { "" } }
                                        on:click=move |_| { let s = audit_ui_step.get(); if s == 3 || s == 4 || s == 5 { audit_ui_step.set(3); } }>
                                        <div class=move || {
                                            let s = audit_ui_step.get();
                                            if s == 4 || s == 5 { "audit-step-num done" } else if s == 3 { "audit-step-num active" } else { "audit-step-num pending" }
                                        }>"3"</div>
                                        <span class=move || { let s = audit_ui_step.get(); if s == 3 || s == 4 || s == 5 { "audit-step-label active" } else { "audit-step-label muted" } }>"Data requests"</span>
                                    </div>
                                    <span class="audit-step-sep">"›"</span>
                                    <div class="audit-step-crumb"
                                        style=move || if audit_ui_step.get() == 5 { "cursor:pointer" } else { "" }
                                        on:click=move |_| { if audit_ui_step.get() == 5 { audit_ui_step.set(4); } }>
                                        <div class=move || { let s = audit_ui_step.get(); if s == 5 { "audit-step-num done" } else if s == 4 { "audit-step-num active" } else { "audit-step-num pending" } }>"4"</div>
                                        <span class=move || { if audit_ui_step.get() == 4 || audit_ui_step.get() == 5 { "audit-step-label active" } else { "audit-step-label muted" } }>"Data collection"</span>
                                    </div>
                                    <span class="audit-step-sep">"›"</span>
                                    <div class="audit-step-crumb">
                                        <div class="audit-step-num pending">"5"</div>
                                        <span class="audit-step-label muted">"Workpapers"</span>
                                    </div>
                                </div>
                            </div>

                            // ── Step 2: Audit plan ────────────────────────────
                            {move || (audit_ui_step.get() == 2).then(|| view! {
                                <step2::Step2View
                                    audit_plan=audit_plan
                                    audit_ui_step=audit_ui_step
                                    plan_needs_regen=plan_needs_regen
                                    sop_analyzing=sop_analyzing
                                    project_files=project_files
                                    status=status
                                    setup_scope=setup_scope
                                />
                            })}

                            // ── Step 3: Data requests ─────────────────────────
                            {move || (audit_ui_step.get() == 3).then(|| view! {
                                <step3::Step3View
                                    audit_plan=audit_plan
                                    audit_ui_step=audit_ui_step
                                    status=status
                                />
                            })}

                            // ── Step 4: Data collection ────────────────────────
                            {move || (audit_ui_step.get() == 4).then(|| view! {
                                <step4::Step4View
                                    audit_plan=audit_plan
                                    audit_ui_step=audit_ui_step
                                    status=status
                                />
                            })}

                            // ── Step 1: Audit setup ───────────────────────────
                            {move || (audit_ui_step.get() == 1).then(|| view! {
                                <step1::AuditSetupView
                                    setup_standards=setup_standards
                                    setup_scope=setup_scope
                                    setup_new_std=setup_new_std
                                    setup_new_scope=setup_new_scope
                                    ai_setup_prompt=ai_setup_prompt
                                    ai_setup_loading=ai_setup_loading
                                    ai_setup_summary=ai_setup_summary
                                    ai_setup_err=ai_setup_err
                                    sop_extracting=sop_extracting
                                    sop_analyzing=sop_analyzing
                                    project_files=project_files
                                    audit_plan=audit_plan
                                    plan_needs_regen=plan_needs_regen
                                    audit_ui_step=audit_ui_step
                                    status=status
                                    on_add_file=on_add_file
                                />
                            })}

                        </div>
                    }
                })}

            </div> // proj-main
        </div> // projects-shell
    }
}
