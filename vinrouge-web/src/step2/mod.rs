use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::types::{AuditProcessWithControls, Control, ProjectFile};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, ask_ollama_json};

fn build_regen_prompt(scope: &[String], sop_text: &str) -> String {
    let base = crate::step1::prompts::ANALYZE_SOP;
    if scope.is_empty() {
        return format!("{base}\n\n{sop_text}");
    }
    let scope_list = scope.iter()
        .map(|s| format!("- {s}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{base}\n\nIMPORTANT: You MUST generate exactly one process entry for EACH of the following \
processes — do not skip, merge, or add extras:\n{scope_list}\n\nSOP TEXT:\n{sop_text}"
    )
}

// ── Step2View ─────────────────────────────────────────────────────────────────

#[component]
pub fn Step2View(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    plan_needs_regen: RwSignal<bool>,
    sop_analyzing: RwSignal<Option<String>>,
    project_files: RwSignal<Vec<ProjectFile>>,
    status: RwSignal<String>,
    setup_scope: RwSignal<Vec<String>>,
) -> impl IntoView {
    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
            // Regen banner (only shown when plan_needs_regen)
            {move || plan_needs_regen.get().then(|| view! {
                <div style="display:flex;align-items:center;gap:10px;padding:8px 14px;background:rgba(139,26,42,0.18);border-bottom:1px solid var(--w-border-2);font-size:12px;color:var(--w-text-2);flex-shrink:0">
                    <svg width="13" height="13" viewBox="0 0 14 14" fill="none" style="flex-shrink:0">
                        <circle cx="7" cy="7" r="6" stroke="currentColor" stroke-width="1.2"/>
                        <path d="M7 4v3.5M7 9.5h.01" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                    </svg>
                    "Setup changed since last generation."
                    <button style="margin-left:auto;padding:4px 12px;font-size:11px;border-radius:4px;border:none;background:var(--w-accent);color:#fff;cursor:pointer;font-family:var(--font)"
                        prop:disabled=move || sop_analyzing.get().is_some()
                        on:click=move |_| {
                            let files = project_files.get();
                            let sop = files.into_iter().find(|f| f.file_type == "pdf" || f.file_type == "txt");
                            if let Some(f) = sop {
                                let fid = f.id.clone();
                                let fname = f.name.clone();
                                let scope = setup_scope.get_untracked();
                                sop_analyzing.set(Some(fid.clone()));
                                spawn_local(async move {
                                    let text = match tauri_invoke_args::<String>("read_project_file", serde_json::json!({ "fileId": fid.clone() })).await {
                                        Ok(t) => t, Err(e) => { status.set(format!("Read error: {e}")); sop_analyzing.set(None); return; }
                                    };
                                    let prompt = build_regen_prompt(&scope, &text);
                                    let json_str = match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                                        Ok(s) => s, Err(e) => { status.set(format!("Ollama error: {e}")); sop_analyzing.set(None); return; }
                                    };
                                    if let Err(e) = tauri_invoke_args::<()>("save_audit_plan", serde_json::json!({ "sopFileId": fid, "processesJson": json_str })).await {
                                        status.set(format!("Save error: {e}")); sop_analyzing.set(None); return;
                                    }
                                    if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
                                        audit_plan.set(p);
                                    }
                                    plan_needs_regen.set(false);
                                    sop_analyzing.set(None);
                                    status.set(format!("Audit plan regenerated for \"{}\"", fname));
                                });
                            }
                        }
                    >
                        {move || if sop_analyzing.get().is_some() { "Regenerating..." } else { "Regenerate plan" }}
                    </button>
                    <button style="padding:4px 10px;font-size:11px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-3);cursor:pointer;font-family:var(--font)"
                        on:click=move |_| plan_needs_regen.set(false)>"Dismiss"</button>
                </div>
            })}

            // Scrollable audit plan area — reactive to audit_plan signal
            <div style="flex:1;overflow-y:auto">
                {move || {
                    let plan = audit_plan.get();
                    if plan.is_empty() {
                        view! {
                            <p class="audit-empty">"Generating audit plan..."</p>
                        }.into_any()
                    } else {
                        view! { <AuditPlanView plan=plan audit_ui_step=audit_ui_step /> }.into_any()
                    }
                }}
            </div>

            // Footer
            <div style="flex-shrink:0;padding:10px 14px;border-top:1px solid var(--w-border);background:var(--w-surface-2);display:flex;align-items:center;gap:8px">
                <button style="padding:6px 14px;font-size:12px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-2);cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                    on:click=move |_| { audit_ui_step.set(1); }
                >
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                        <path d="M10 6 L2 6 M5.5 2.5 L2 6 L5.5 9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    "Back to setup"
                </button>
                <button
                    style="padding:6px 14px;font-size:12px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-2);cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                    prop:disabled=move || sop_analyzing.get().is_some()
                    on:click=move |_| {
                        let files = project_files.get();
                        let sop = files.into_iter().find(|f| f.file_type == "pdf" || f.file_type == "txt");
                        if let Some(f) = sop {
                            let fid   = f.id.clone();
                            let fname = f.name.clone();
                            let scope = setup_scope.get_untracked();
                            sop_analyzing.set(Some(fid.clone()));
                            spawn_local(async move {
                                let text = match tauri_invoke_args::<String>(
                                    "read_project_file",
                                    serde_json::json!({ "fileId": fid.clone() }),
                                ).await {
                                    Ok(t) => t,
                                    Err(e) => { status.set(format!("Read error: {e}")); sop_analyzing.set(None); return; }
                                };
                                let prompt = build_regen_prompt(&scope, &text);
                                let json_str = match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                                    Ok(s) => s,
                                    Err(e) => { status.set(format!("Ollama error: {e}")); sop_analyzing.set(None); return; }
                                };
                                if let Err(e) = tauri_invoke_args::<()>(
                                    "save_audit_plan",
                                    serde_json::json!({ "sopFileId": fid, "processesJson": json_str }),
                                ).await {
                                    status.set(format!("Save error: {e}")); sop_analyzing.set(None); return;
                                }
                                if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
                                    audit_plan.set(p);
                                }
                                plan_needs_regen.set(false);
                                sop_analyzing.set(None);
                                status.set(format!("Audit plan regenerated for \"{}\"", fname));
                            });
                        } else {
                            status.set("No SOP file found — upload a PDF or TXT file in setup first.".into());
                        }
                    }
                >
                    <svg width="11" height="11" viewBox="0 0 14 14" fill="none">
                        <path d="M12.5 7A5.5 5.5 0 1 1 7 1.5c1.8 0 3.4.87 4.4 2.2M12.5 1.5v3.7H8.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    {move || if sop_analyzing.get().is_some() { "Regenerating..." } else { "Regenerate plan" }}
                </button>
                <button style="margin-left:auto;padding:6px 16px;font-size:12px;border-radius:4px;border:none;background:var(--w-accent);color:#fff;cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                    on:click=move |_| { audit_ui_step.set(3); }
                >
                    "Next — Data requests"
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                        <path d="M2 6 L10 6 M6.5 2.5 L10 6 L6.5 9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </button>
            </div>
        </div>
    }
}

// ── AuditPlanView ─────────────────────────────────────────────────────────────

#[component]
fn AuditPlanView(
    plan: Vec<AuditProcessWithControls>,
    audit_ui_step: RwSignal<u8>,
) -> impl IntoView {
    let count = plan.len();
    view! {
        <div class="audit-plan">
            <div class="audit-plan-header">
                <span class="audit-plan-title">"AUDIT PLAN"</span>
                <span class="audit-plan-badge">{count} " processes"</span>
            </div>
            {plan.into_iter().map(|proc| view! {
                <AuditProcessCard proc=proc />
            }).collect_view()}
        </div>
    }
}

// ── AuditProcessCard ──────────────────────────────────────────────────────────

#[component]
fn AuditProcessCard(proc: AuditProcessWithControls) -> impl IntoView {
    let open: RwSignal<bool> = RwSignal::new(true);

    let proc_id: RwSignal<String>            = RwSignal::new(proc.id.clone());
    let pname_sig: RwSignal<String>          = RwSignal::new(proc.process_name.clone());
    let pdesc_sig: RwSignal<String>          = RwSignal::new(proc.description.clone());
    let prompt_sig: RwSignal<String>         = RwSignal::new(proc.audit_prompt.clone());
    let controls_sig: RwSignal<Vec<Control>> = RwSignal::new(proc.controls.clone());

    let ai_loading: RwSignal<bool>           = RwSignal::new(false);
    let ai_status:  RwSignal<Option<String>> = RwSignal::new(None);

    let edit_name: RwSignal<bool> = RwSignal::new(false);
    let edit_desc: RwSignal<bool> = RwSignal::new(false);

    view! {
        <div class="audit-process-card">
            <div class="audit-process-header" on:click=move |_| open.update(|v| *v = !*v)>
                <span class=move || if open.get() {
                    "audit-process-chevron open"
                } else {
                    "audit-process-chevron"
                }>"▶"</span>
                <div class="audit-process-name-wrap">
                    {move || if edit_name.get() {
                        view! {
                            <input
                                class="editable-input"
                                prop:value=move || pname_sig.get()
                                on:input=move |ev| pname_sig.set(event_target_value(&ev))
                                on:blur=move |_| {
                                    edit_name.set(false);
                                    let v = pname_sig.get();
                                    let pid = proc_id.get_untracked();
                                    spawn_local(async move {
                                        let _ = tauri_invoke_args::<()>(
                                            "update_process_field",
                                            serde_json::json!({
                                                "processId": pid,
                                                "field": "process_name",
                                                "value": v,
                                            }),
                                        ).await;
                                    });
                                }
                                on:click=move |ev| { ev.stop_propagation(); }
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="audit-process-name"
                                on:click=move |ev| { ev.stop_propagation(); edit_name.set(true); }>
                                {move || pname_sig.get()}
                            </div>
                        }.into_any()
                    }}
                    {move || if edit_desc.get() {
                        view! {
                            <input
                                class="editable-input"
                                style="margin-top:2px"
                                prop:value=move || pdesc_sig.get()
                                on:input=move |ev| pdesc_sig.set(event_target_value(&ev))
                                on:blur=move |_| {
                                    edit_desc.set(false);
                                    let v = pdesc_sig.get();
                                    let pid = proc_id.get_untracked();
                                    spawn_local(async move {
                                        let _ = tauri_invoke_args::<()>(
                                            "update_process_field",
                                            serde_json::json!({
                                                "processId": pid,
                                                "field": "description",
                                                "value": v,
                                            }),
                                        ).await;
                                    });
                                }
                                on:click=move |ev| { ev.stop_propagation(); }
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="audit-process-desc"
                                on:click=move |ev| { ev.stop_propagation(); edit_desc.set(true); }>
                                {move || pdesc_sig.get()}
                            </div>
                        }.into_any()
                    }}
                </div>
            </div>

            {move || open.get().then(|| {
                view! {
                    <div class="audit-process-body">
                        {move || {
                            let rows = controls_sig.get();
                            if rows.is_empty() {
                                view! {
                                    <p class="audit-empty">"No controls — use Add control below."</p>
                                }.into_any()
                            } else {
                                view! {
                                    <table class="controls-table">
                                        <thead>
                                            <tr>
                                                <th>"Ref"</th>
                                                <th>"Control objective"</th>
                                                <th>"How it operates"</th>
                                                <th>"Test procedure"</th>
                                                <th>"Risk"</th>
                                                <th></th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {rows.into_iter().map(|c| view! {
                                                <ControlRow ctrl=c controls_sig=controls_sig />
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                }.into_any()
                            }
                        }}

                        // Add control button
                        <div style="padding:6px 0 2px">
                            <button
                                class="add-control-btn"
                                on:click=move |_| {
                                    let pid = proc_id.get_untracked();
                                    let next_ref = format!("C-{:02}", controls_sig.get().len() + 1);
                                    spawn_local(async move {
                                        match tauri_invoke_args::<Control>(
                                            "add_control",
                                            serde_json::json!({
                                                "processId": pid,
                                                "controlRef": next_ref,
                                                "controlObjective": "",
                                                "controlDescription": "",
                                                "testProcedure": "",
                                                "riskLevel": "Medium",
                                            }),
                                        ).await {
                                            Ok(ctrl) => controls_sig.update(|v| v.push(ctrl)),
                                            Err(e) => leptos::logging::warn!("add_control error: {e}"),
                                        }
                                    });
                                }
                            >
                                <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                                    <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                                </svg>
                                " Add control"
                            </button>
                        </div>

                        // AI instruction
                        <div class="audit-section-prompt">
                            <label class="audit-section-prompt-label">
                                "AI instruction"
                            </label>
                            <div class="audit-section-prompt-row">
                                <textarea
                                    class="audit-section-prompt-textarea"
                                    placeholder="Instruct the AI to update this section — e.g. \"Add a control for segregation of duties\" or \"Raise all risks to High\"..."
                                    prop:value=move || prompt_sig.get()
                                    on:input=move |ev| prompt_sig.set(event_target_value(&ev))
                                    on:blur=move |_| {
                                        let v = prompt_sig.get();
                                        let id = proc_id.get_untracked();
                                        spawn_local(async move {
                                            let _ = tauri_invoke_args::<()>(
                                                "update_process_field",
                                                serde_json::json!({
                                                    "processId": id,
                                                    "field": "audit_prompt",
                                                    "value": v,
                                                }),
                                            ).await;
                                        });
                                    }
                                />
                                <button
                                    class="audit-section-send-btn"
                                    prop:disabled=move || ai_loading.get()
                                    on:click=move |_| {
                                        let instruction = prompt_sig.get();
                                        if instruction.trim().is_empty() || ai_loading.get() { return; }

                                        let name  = pname_sig.get();
                                        let desc  = pdesc_sig.get();
                                        let ctrls = controls_sig.get();
                                        let mut ctx = format!(
                                            "Current process:\nName: {name}\nDescription: {desc}\nControls:\n"
                                        );
                                        for c in &ctrls {
                                            ctx.push_str(&format!(
                                                "- {} | {} | {} | {} | {}\n",
                                                c.control_ref, c.control_objective,
                                                c.control_description, c.test_procedure, c.risk_level
                                            ));
                                        }
                                        ctx.push_str(&format!("\nUser instruction: {instruction}"));

                                        let full_prompt = format!(
                                            "{}{ctx}",
                                            vinrouge::audit_prompts::UPDATE_SECTION
                                        );

                                        ai_loading.set(true);
                                        ai_status.set(None);

                                        spawn_local(async move {
                                            let raw = match ask_ollama_json(
                                                OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &full_prompt
                                            ).await {
                                                Ok(s)  => s,
                                                Err(e) => {
                                                    ai_status.set(Some(format!("Error: {e}")));
                                                    ai_loading.set(false);
                                                    return;
                                                }
                                            };

                                            let v: serde_json::Value = match serde_json::from_str(&raw) {
                                                Ok(v)  => v,
                                                Err(e) => {
                                                    ai_status.set(Some(format!("Parse error: {e}")));
                                                    ai_loading.set(false);
                                                    return;
                                                }
                                            };

                                            if let Some(new_name) = v["process_name"].as_str() {
                                                pname_sig.set(new_name.to_string());
                                                let pid = proc_id.get_untracked();
                                                let val = new_name.to_string();
                                                let _ = tauri_invoke_args::<()>("update_process_field",
                                                    serde_json::json!({"processId":pid,"field":"process_name","value":val})).await;
                                            }

                                            if let Some(new_desc) = v["description"].as_str() {
                                                pdesc_sig.set(new_desc.to_string());
                                                let pid = proc_id.get_untracked();
                                                let val = new_desc.to_string();
                                                let _ = tauri_invoke_args::<()>("update_process_field",
                                                    serde_json::json!({"processId":pid,"field":"description","value":val})).await;
                                            }

                                            if let Some(arr) = v["controls"].as_array() {
                                                let current = controls_sig.get();

                                                // Delete controls absent from AI response
                                                let ai_refs: Vec<&str> = arr.iter()
                                                    .filter_map(|c| c["control_ref"].as_str())
                                                    .collect();
                                                for ctrl in &current {
                                                    if !ai_refs.contains(&ctrl.control_ref.as_str()) {
                                                        let id = ctrl.id.clone();
                                                        let _ = tauri_invoke_args::<()>("delete_control",
                                                            serde_json::json!({"controlId": id})).await;
                                                    }
                                                }

                                                // Build the new ordered list
                                                let mut new_controls: Vec<crate::types::Control> = vec![];
                                                for ctrl_json in arr.iter() {
                                                    let ai_ref = ctrl_json["control_ref"].as_str().unwrap_or("");
                                                    if let Some(existing) = current.iter().find(|c| c.control_ref == ai_ref) {
                                                        // Update existing control
                                                        let mut ctrl = existing.clone();
                                                        let id = ctrl.id.clone();
                                                        let fields = [
                                                            ("control_ref",         ctrl_json["control_ref"].as_str()),
                                                            ("control_objective",   ctrl_json["control_objective"].as_str()),
                                                            ("control_description", ctrl_json["control_description"].as_str()),
                                                            ("test_procedure",      ctrl_json["test_procedure"].as_str()),
                                                            ("risk_level",          ctrl_json["risk_level"].as_str()),
                                                        ];
                                                        for (field, maybe_val) in fields {
                                                            if let Some(val) = maybe_val {
                                                                match field {
                                                                    "control_ref"         => ctrl.control_ref = val.to_string(),
                                                                    "control_objective"   => ctrl.control_objective = val.to_string(),
                                                                    "control_description" => ctrl.control_description = val.to_string(),
                                                                    "test_procedure"      => ctrl.test_procedure = val.to_string(),
                                                                    "risk_level"          => ctrl.risk_level = val.to_string(),
                                                                    _ => {}
                                                                }
                                                                let cid = id.clone();
                                                                let fv = val.to_string();
                                                                let _ = tauri_invoke_args::<()>("update_control_field",
                                                                    serde_json::json!({"controlId":cid,"field":field,"value":fv})).await;
                                                            }
                                                        }
                                                        new_controls.push(ctrl);
                                                    } else {
                                                        // Add new control
                                                        let pid = proc_id.get_untracked();
                                                        match tauri_invoke_args::<crate::types::Control>("add_control",
                                                            serde_json::json!({
                                                                "processId":          pid,
                                                                "controlRef":         ctrl_json["control_ref"].as_str().unwrap_or(""),
                                                                "controlObjective":   ctrl_json["control_objective"].as_str().unwrap_or(""),
                                                                "controlDescription": ctrl_json["control_description"].as_str().unwrap_or(""),
                                                                "testProcedure":      ctrl_json["test_procedure"].as_str().unwrap_or(""),
                                                                "riskLevel":          ctrl_json["risk_level"].as_str().unwrap_or("Medium"),
                                                            }),
                                                        ).await {
                                                            Ok(ctrl) => new_controls.push(ctrl),
                                                            Err(e)   => leptos::logging::warn!("add_control error: {e}"),
                                                        }
                                                    }
                                                }

                                                controls_sig.set(new_controls);
                                            }

                                            ai_status.set(Some("Section updated.".into()));
                                            ai_loading.set(false);
                                        });
                                    }
                                >
                                    {move || if ai_loading.get() {
                                        view! {
                                            <svg width="13" height="13" viewBox="0 0 14 14" fill="none" style="animation:spin 1s linear infinite">
                                                <circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.4" stroke-dasharray="22" stroke-dashoffset="8" stroke-linecap="round"/>
                                            </svg>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
                                                <path d="M1.5 7h11M8 2.5l4.5 4.5L8 11.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                        }.into_any()
                                    }}
                                </button>
                            </div>
                            {move || ai_status.get().map(|s| {
                                let is_err = s.starts_with("Error") || s.starts_with("Parse");
                                view! {
                                    <div class=if is_err { "audit-ai-error" } else { "audit-ai-ok" }>
                                        {s}
                                    </div>
                                }
                            })}
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

// ── ControlRow ────────────────────────────────────────────────────────────────

#[component]
fn ControlRow(ctrl: Control, controls_sig: RwSignal<Vec<Control>>) -> impl IntoView {
    let ctrl_id  = ctrl.id.clone();
    let cid_ref  = ctrl.id.clone();
    let cid_obj  = ctrl.id.clone();
    let cid_desc = ctrl.id.clone();
    let cid_test = ctrl.id.clone();
    let cid_risk = ctrl.id.clone();

    let ref_sig:  RwSignal<String> = RwSignal::new(ctrl.control_ref.clone());
    let obj_sig:  RwSignal<String> = RwSignal::new(ctrl.control_objective.clone());
    let desc_sig: RwSignal<String> = RwSignal::new(ctrl.control_description.clone());
    let test_sig: RwSignal<String> = RwSignal::new(ctrl.test_procedure.clone());
    let risk_sig: RwSignal<String> = RwSignal::new(ctrl.risk_level.clone());

    let risk_class = move || match risk_sig.get().as_str() {
        "High" => "risk-badge high",
        "Low"  => "risk-badge low",
        _      => "risk-badge medium",
    };

    view! {
        <tr>
            <td class="ctrl-ref">
                <input class="editable-input"
                    style="width:46px"
                    prop:value=move || ref_sig.get()
                    on:input=move |ev| ref_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = ref_sig.get();
                        let id = cid_ref.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_ref","value":v})).await;
                        });
                    }
                />
            </td>
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || obj_sig.get()
                    on:input=move |ev| obj_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = obj_sig.get();
                        let id = cid_obj.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_objective","value":v})).await;
                        });
                    }
                />
            </td>
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || desc_sig.get()
                    on:input=move |ev| desc_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = desc_sig.get();
                        let id = cid_desc.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_description","value":v})).await;
                        });
                    }
                />
            </td>
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || test_sig.get()
                    on:input=move |ev| test_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = test_sig.get();
                        let id = cid_test.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"test_procedure","value":v})).await;
                        });
                    }
                />
            </td>
            <td class="ctrl-risk">
                <select class="risk-select"
                    prop:value=move || risk_sig.get()
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        risk_sig.set(v.clone());
                        let id = cid_risk.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"risk_level","value":v})).await;
                        });
                    }
                >
                    <option value="High">"High"</option>
                    <option value="Medium">"Medium"</option>
                    <option value="Low">"Low"</option>
                </select>
            </td>
            // Delete button
            <td class="ctrl-action">
                <button
                    class="ctrl-delete-btn"
                    title="Remove control"
                    on:click=move |_| {
                        let id = ctrl_id.clone();
                        controls_sig.update(|v| v.retain(|c| c.id != id));
                        let id2 = ctrl_id.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>(
                                "delete_control",
                                serde_json::json!({ "controlId": id2 }),
                            ).await;
                        });
                    }
                >
                    <svg width="12" height="12" viewBox="0 0 14 14" fill="none">
                        <path d="M2 4h10M5 4V2.5h4V4M6 6.5v4M8 6.5v4M3 4l.7 7.5h6.6L11 4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </button>
            </td>
        </tr>
    }
}
