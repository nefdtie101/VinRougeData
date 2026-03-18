pub mod prompts;

use leptos::callback::{Callable, UnsyncCallback};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_ollama_json, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, ProjectFile};

#[component]
pub fn AuditSetupView(
    setup_standards: RwSignal<Vec<(String, bool)>>,
    setup_scope: RwSignal<Vec<String>>,
    setup_new_std: RwSignal<String>,
    setup_new_scope: RwSignal<String>,
    ai_setup_prompt: RwSignal<String>,
    ai_setup_loading: RwSignal<bool>,
    ai_setup_summary: RwSignal<Option<String>>,
    ai_setup_err: RwSignal<bool>,
    sop_extracting: RwSignal<bool>,
    sop_analyzing: RwSignal<Option<String>>,
    project_files: RwSignal<Vec<ProjectFile>>,
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    plan_needs_regen: RwSignal<bool>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
    #[prop(into)] on_add_file: UnsyncCallback<(web_sys::MouseEvent,)>,
) -> impl IntoView {
    view! {
        // ── Scrollable content ────────────────────────────────────────────────
        <div class="audit-setup-content">
            <div class="audit-setup-header">
                <h1>"Audit setup"</h1>
                <p>"Upload the client's process SOP and configure the audit scope. The AI will use this to generate the audit plan in Step 2."</p>
            </div>

            // SOP document
            <div class="audit-setup-section">
                <div class="audit-setup-label">"SOP document"</div>
                {move || {
                    let files = project_files.get();
                    let sop = files.into_iter().find(|f| f.file_type == "pdf" || f.file_type == "txt");
                    if let Some(f) = sop {
                        view! {
                            <div class="sop-file-zone has-file">
                                <div style="display:flex;align-items:center;gap:12px;justify-content:center">
                                    <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
                                        <rect x="4" y="2" width="16" height="22" rx="2" stroke="var(--w-success)" stroke-width="1.3"/>
                                        <path d="M16 2 L16 8 L22 8" stroke="var(--w-success)" stroke-width="1.3" stroke-linejoin="round"/>
                                        <line x1="8" y1="13" x2="18" y2="13" stroke="var(--w-success)" stroke-width="1.2" stroke-linecap="round"/>
                                        <line x1="8" y1="17" x2="15" y2="17" stroke="var(--w-success)" stroke-width="1.2" stroke-linecap="round"/>
                                    </svg>
                                    <div style="text-align:left">
                                        <div style="font-size:13px;font-weight:500;color:var(--w-text)">{f.name}</div>
                                        <div style="font-size:11px;color:var(--w-text-3);margin-bottom:2px">{f.file_type.to_uppercase()} " · Uploaded"</div>
                                        {move || if sop_extracting.get() {
                                            view! { <span style="font-size:11px;color:var(--w-text-3)">"Reading document..."</span> }.into_any()
                                        } else {
                                            view! { <span style="font-size:11px;color:var(--w-success);cursor:pointer" on:click=move |ev| on_add_file.run((ev,))>"Replace file"</span> }.into_any()
                                        }}
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="sop-file-zone" on:click=move |ev| on_add_file.run((ev,))>
                                <svg width="28" height="28" viewBox="0 0 28 28" fill="none" style="margin:0 auto 8px;display:block;color:var(--w-text-3)">
                                    <rect x="4" y="2" width="16" height="22" rx="2" stroke="currentColor" stroke-width="1.3"/>
                                    <path d="M16 2 L16 8 L22 8" stroke="currentColor" stroke-width="1.3"/>
                                    <line x1="14" y1="10" x2="14" y2="18" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                                    <line x1="10" y1="14" x2="18" y2="14" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                                </svg>
                                <div style="font-size:13px;font-weight:500;color:var(--w-text);margin-bottom:2px">"Upload SOP document"</div>
                                <div style="font-size:11px;color:var(--w-text-3)">"PDF or TXT file"</div>
                            </div>
                        }.into_any()
                    }
                }}
            </div>

            // Applicable standards
            <div class="audit-setup-section">
                <div class="audit-setup-label">"Applicable standards"</div>
                <div class="audit-std-grid">
                    {move || setup_standards.get().into_iter().enumerate().map(|(i, (label, selected))| {
                        view! {
                            <div
                                class=if selected { "audit-std-chip selected" } else { "audit-std-chip" }
                                on:click=move |_| {
                                    setup_standards.update(|v| {
                                        if let Some(s) = v.get_mut(i) { s.1 = !s.1; }
                                    });
                                    if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                                }
                            >
                                <svg class="chip-check" width="10" height="10" viewBox="0 0 10 10" fill="none">
                                    <polyline points="1.5,5 4,7.5 8.5,2.5" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                                {label}
                            </div>
                        }
                    }).collect_view()}
                </div>
                <div class="audit-add-row">
                    <input class="audit-add-input"
                        placeholder="Add a standard (e.g. POPIA, COBIT)"
                        prop:value=move || setup_new_std.get()
                        on:input=move |ev| setup_new_std.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                let val = setup_new_std.get().trim().to_string();
                                if !val.is_empty() {
                                    setup_standards.update(|v| v.push((val, true)));
                                    setup_new_std.set(String::new());
                                    if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                                }
                            }
                        }
                    />
                    <button class="audit-btn-add"
                        on:click=move |_| {
                            let val = setup_new_std.get().trim().to_string();
                            if !val.is_empty() {
                                setup_standards.update(|v| v.push((val, true)));
                                setup_new_std.set(String::new());
                                if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                            }
                        }
                    >"+ Add"</button>
                </div>
            </div>

            // Audit scope
            <div class="audit-setup-section">
                <div class="audit-setup-label">"Audit scope — processes in scope"</div>
                <div class="audit-scope-list">
                    {move || setup_scope.get().into_iter().enumerate().map(|(i, item)| {
                        view! {
                            <div class="audit-scope-item">
                                <div class="audit-scope-dot"></div>
                                <span style="flex:1">{item}</span>
                                <button class="audit-scope-remove"
                                    on:click=move |_| {
                                        setup_scope.update(|v| { v.remove(i); });
                                        if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                                    }
                                >"×"</button>
                            </div>
                        }
                    }).collect_view()}
                </div>
                <div class="audit-add-row">
                    <input class="audit-add-input"
                        placeholder="Add a process (e.g. Fleet maintenance)"
                        prop:value=move || setup_new_scope.get()
                        on:input=move |ev| setup_new_scope.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                let val = setup_new_scope.get().trim().to_string();
                                if !val.is_empty() {
                                    setup_scope.update(|v| v.push(val));
                                    setup_new_scope.set(String::new());
                                    if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                                }
                            }
                        }
                    />
                    <button class="audit-btn-add"
                        on:click=move |_| {
                            let val = setup_new_scope.get().trim().to_string();
                            if !val.is_empty() {
                                setup_scope.update(|v| v.push(val));
                                setup_new_scope.set(String::new());
                                if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                            }
                        }
                    >"+ Add"</button>
                </div>
            </div>

            // AI prompt box
            <div class="audit-ai-box">
                <div class="audit-ai-header">
                    <div class="audit-ai-dot">
                        <svg viewBox="0 0 10 10" fill="none" width="8" height="8">
                            <circle cx="5" cy="5" r="3.5" stroke="#fff" stroke-width="1.2"/>
                            <circle cx="5" cy="5" r="1.2" fill="#fff"/>
                        </svg>
                    </div>
                    <span>"Tell the AI how to update this setup — it will adjust the standards and scope"</span>
                </div>
                <div class="audit-ai-input-row">
                    <textarea class="audit-ai-textarea" rows="2"
                        placeholder="e.g. This is a POPIA compliance audit for a car rental company..."
                        prop:value=move || ai_setup_prompt.get()
                        prop:disabled=move || ai_setup_loading.get()
                        on:input=move |ev| ai_setup_prompt.set(event_target_value(&ev))
                    />
                    <button class="audit-ai-send"
                        prop:disabled=move || ai_setup_loading.get()
                        on:click=move |_| {
                            let prompt = ai_setup_prompt.get().trim().to_string();
                            if prompt.is_empty() { return; }
                            ai_setup_loading.set(true);
                            ai_setup_summary.set(None);
                            ai_setup_err.set(false);
                            let stds: Vec<String> = setup_standards.get()
                                .iter().filter(|(_, s)| *s).map(|(n, _)| n.clone()).collect();
                            let scope: Vec<String> = setup_scope.get();
                            let full = format!(
                                "You are an audit config assistant. Respond in JSON only.\n\
                                 Return: {{\"summary\":\"...\",\"standards_add\":[],\"standards_remove\":[],\"scope_add\":[],\"scope_remove\":[]}}\n\
                                 Selected standards: {}\nScope: {}\n\nInstruction: {}",
                                stds.join(", "), scope.join("; "), prompt
                            );
                            spawn_local(async move {
                                match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &full).await {
                                    Ok(raw) => {
                                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                            if let Some(arr) = v["standards_add"].as_array() {
                                                for n in arr {
                                                    if let Some(s) = n.as_str() {
                                                        let s = s.to_string();
                                                        setup_standards.update(|v| {
                                                            if let Some(e) = v.iter_mut().find(|(x,_)| x.to_lowercase() == s.to_lowercase()) { e.1 = true; }
                                                            else { v.push((s, true)); }
                                                        });
                                                    }
                                                }
                                            }
                                            if let Some(arr) = v["standards_remove"].as_array() {
                                                for n in arr {
                                                    if let Some(s) = n.as_str() {
                                                        let sl = s.to_lowercase();
                                                        setup_standards.update(|v| {
                                                            if let Some(e) = v.iter_mut().find(|(x,_)| x.to_lowercase() == sl) { e.1 = false; }
                                                        });
                                                    }
                                                }
                                            }
                                            if let Some(arr) = v["scope_add"].as_array() {
                                                for n in arr {
                                                    if let Some(s) = n.as_str() {
                                                        let s = s.to_string();
                                                        setup_scope.update(|v| {
                                                            if !v.iter().any(|x| x.to_lowercase() == s.to_lowercase()) { v.push(s); }
                                                        });
                                                    }
                                                }
                                            }
                                            if let Some(arr) = v["scope_remove"].as_array() {
                                                for n in arr {
                                                    if let Some(s) = n.as_str() {
                                                        let sl = s.to_lowercase();
                                                        setup_scope.update(|v| v.retain(|x| x.to_lowercase() != sl));
                                                    }
                                                }
                                            }
                                            let summary = v["summary"].as_str().unwrap_or("Changes applied").to_string();
                                            ai_setup_summary.set(Some(summary));
                                            ai_setup_prompt.set(String::new());
                                            if !audit_plan.get().is_empty() { plan_needs_regen.set(true); }
                                        } else {
                                            ai_setup_err.set(true);
                                        }
                                    }
                                    Err(_) => ai_setup_err.set(true),
                                }
                                ai_setup_loading.set(false);
                            });
                        }
                    >
                        <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                            <path d="M1 6 L11 6 M7 2 L11 6 L7 10" stroke="#fff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                        {move || if ai_setup_loading.get() { "Thinking..." } else { "Update" }}
                    </button>
                </div>
                {move || ai_setup_loading.get().then(|| view! {
                    <div class="audit-ai-response">
                        <div style="display:flex;gap:4px;align-items:center">
                            <span style="width:4px;height:4px;border-radius:50%;background:var(--w-success);animation:blink 1.2s infinite;display:inline-block"></span>
                            <span style="width:4px;height:4px;border-radius:50%;background:var(--w-success);animation:blink 1.2s 0.2s infinite;display:inline-block"></span>
                            <span style="width:4px;height:4px;border-radius:50%;background:var(--w-success);animation:blink 1.2s 0.4s infinite;display:inline-block"></span>
                        </div>
                    </div>
                })}
                {move || ai_setup_summary.get().map(|s| view! {
                    <div class="audit-ai-response">
                        <div style="color:var(--w-text)">{s}</div>
                        <div class="audit-applied-badge">
                            <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                <polyline points="1.5,6 4.5,9.5 10.5,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                            </svg>
                            "Changes applied"
                        </div>
                    </div>
                })}
                {move || ai_setup_err.get().then(|| view! {
                    <div class="audit-ai-response" style="color:#f87171">
                        "Could not reach AI — check Ollama is running"
                    </div>
                })}
            </div>
        </div>

        // ── Bottom bar: Generate / Next ───────────────────────────────────────
        <div class="audit-approval-bar">
            <div style="font-size:12px;color:var(--w-text-3);flex:1;min-width:0">
                {move || {
                    let has_sop = project_files.get().iter().any(|f| f.file_type == "pdf" || f.file_type == "txt");
                    if sop_extracting.get() {
                        view! { <span>"Reading SOP and extracting setup..."</span> }.into_any()
                    } else if !has_sop {
                        view! { <span>"Upload a SOP document to get started"</span> }.into_any()
                    } else if audit_plan.get().is_empty() {
                        view! { <span>"Configure standards and scope, then generate the audit plan"</span> }.into_any()
                    } else {
                        view! { <span style="color:var(--w-success)">"Audit plan ready"</span> }.into_any()
                    }
                }}
            </div>
            <div style="display:flex;align-items:center;gap:8px;flex-shrink:0">
                // Generate plan button — only shown when SOP exists and no plan yet
                {move || {
                    let has_sop = project_files.get().iter().any(|f| f.file_type == "pdf" || f.file_type == "txt");
                    let has_plan = !audit_plan.get().is_empty();
                    if has_sop && !has_plan {
                        view! {
                            <button class="audit-proceed-btn"
                                prop:disabled=move || sop_analyzing.get().is_some() || sop_extracting.get()
                                on:click=move |_| {
                                    let files = project_files.get();
                                    let sop = files.into_iter().find(|f| f.file_type == "pdf" || f.file_type == "txt");
                                    if let Some(f) = sop {
                                        let fid = f.id.clone();
                                        let fname = f.name.clone();
                                        sop_analyzing.set(Some(fid.clone()));
                                        status.set("Generating audit plan...".to_string());
                                        spawn_local(async move {
                                            let text = match tauri_invoke_args::<String>("read_project_file", serde_json::json!({ "fileId": fid.clone() })).await {
                                                Ok(t) => t,
                                                Err(e) => { status.set(format!("Read error: {e}")); sop_analyzing.set(None); return; }
                                            };
                                            let prompt = format!("{}\n\n{}", prompts::ANALYZE_SOP, text);
                                            let json_str = match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                                                Ok(s) => s,
                                                Err(e) => { status.set(format!("Ollama error: {e}")); sop_analyzing.set(None); return; }
                                            };
                                            if let Err(e) = tauri_invoke_args::<()>("save_audit_plan", serde_json::json!({ "sopFileId": fid, "processesJson": json_str })).await {
                                                status.set(format!("Save error: {e}")); sop_analyzing.set(None); return;
                                            }
                                            if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
                                                if !p.is_empty() { audit_ui_step.set(2); }
                                                audit_plan.set(p);
                                            }
                                            plan_needs_regen.set(false);
                                            sop_analyzing.set(None);
                                            status.set(format!("Audit plan ready for \"{}\"", fname));
                                        });
                                    }
                                }
                            >
                                {move || if sop_analyzing.get().is_some() { "Generating..." } else { "Generate audit plan" }}
                                {move || sop_analyzing.get().is_none().then(|| view! {
                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                        <path d="M2 6 L10 6 M6.5 2.5 L10 6 L6.5 9.5" stroke="#fff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                                    </svg>
                                })}
                            </button>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
                // Next button — always shown
                <button class="audit-proceed-btn"
                    style="background:var(--w-surface-2);color:var(--w-text-2);border:0.5px solid var(--w-border-2)"
                    on:click=move |_| { audit_ui_step.set(2); }
                >
                    "Next"
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                        <path d="M2 6 L10 6 M6.5 2.5 L10 6 L6.5 9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </button>
            </div>
        </div>
    }
}
