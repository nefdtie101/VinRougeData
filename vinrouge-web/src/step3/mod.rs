use crate::components::{DashedAddButton, ProgressRing, SectionPrompt, Spinner, StatCard};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_ollama_json, ask_ollama_structured, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::step1::prompts::REFINE_PBC_LIST;
use crate::types::{AuditProcessWithControls, PbcGroup, PbcItem};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

// ── Step3View ─────────────────────────────────────────────────────────────────

#[component]
pub fn Step3View(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let groups: RwSignal<Vec<PbcGroup>> = RwSignal::new(vec![]);
    // Separate signal for approval state so toggling doesn't re-render group cards
    let approved_ids: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let list_approved: RwSignal<bool> = RwSignal::new(false);
    let generating: RwSignal<bool> = RwSignal::new(false);
    let gen_error: RwSignal<Option<String>> = RwSignal::new(None);
    let gen_phase: RwSignal<String> = RwSignal::new(String::new());
    let ai_prompt: RwSignal<String> = RwSignal::new(String::new());
    let ai_loading: RwSignal<bool> = RwSignal::new(false);
    let ai_status: RwSignal<Option<String>> = RwSignal::new(None);
    let export_open: RwSignal<bool> = RwSignal::new(false);
    let sync_loading: RwSignal<bool> = RwSignal::new(false);
    let sync_status: RwSignal<Option<String>> = RwSignal::new(None);

    // ── Initial load ──────────────────────────────────────────────────────────
    spawn_local(async move {
        if let Ok(v) =
            tauri_invoke_args::<bool>("get_pbc_list_approved", serde_json::json!({})).await
        {
            list_approved.set(v);
        }
        match tauri_invoke_args::<Vec<PbcGroup>>("list_pbc_groups", serde_json::json!({})).await {
            Ok(g) if g.iter().any(|grp| !grp.items.is_empty()) => {
                let ids: Vec<String> = g
                    .iter()
                    .flat_map(|grp| grp.items.iter())
                    .filter(|i| i.approved)
                    .map(|i| i.id.clone())
                    .collect();
                approved_ids.set(ids);
                groups.set(g);
            }
            _ => {
                generating.set(true);
                gen_error.set(None);
                do_generate(
                    audit_plan,
                    groups,
                    approved_ids,
                    gen_phase,
                    gen_error,
                    generating,
                )
                .await;
            }
        }
    });

    // ── Derived stats ─────────────────────────────────────────────────────────
    let total_items = move || groups.get().iter().flat_map(|g| g.items.iter()).count();
    let approved_count = move || approved_ids.get().len();
    let sql_count = move || {
        groups
            .get()
            .iter()
            .flat_map(|g| g.items.iter())
            .filter(|i| i.item_type == "SQL")
            .count()
    };
    let csv_count = move || {
        groups
            .get()
            .iter()
            .flat_map(|g| g.items.iter())
            .filter(|i| i.item_type == "CSV")
            .count()
    };

    view! {
        <div style="flex:1;min-height:0;display:flex;flex-direction:column;overflow:hidden">

            // ── Header ────────────────────────────────────────────────────────
            <div style="flex-shrink:0;display:flex;align-items:center;justify-content:space-between;padding:8px 14px;border-bottom:1px solid var(--w-border);background:var(--w-surface-2)">
                <span style="font-size:13px;font-weight:500;color:var(--w-text-1)">"Data requests — PBC list"</span>
                <div style="display:flex;gap:8px;align-items:center">
                    // Regenerate button
                    <button
                        style="padding:5px 12px;font-size:12px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-2);cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                        prop:disabled=move || generating.get() || list_approved.get()
                        on:click=move |_| {
                            if generating.get() || list_approved.get() { return; }
                            generating.set(true);
                            gen_error.set(None);
                            groups.set(vec![]);
                            approved_ids.set(vec![]);
                            list_approved.set(false);
                            spawn_local(async move {
                                // Clear first, then generate — sequential in one async block
                                let _ = tauri_invoke_args::<()>("clear_pbc_items", serde_json::json!({})).await;
                                let _ = tauri_invoke_args::<()>("set_pbc_list_approved", serde_json::json!({"approved": false})).await;
                                do_generate(audit_plan, groups, approved_ids, gen_phase, gen_error, generating).await;
                            });
                        }
                    >
                        <svg width="11" height="11" viewBox="0 0 14 14" fill="none">
                            <path d="M12.5 7A5.5 5.5 0 1 1 7 1.5c1.8 0 3.4.87 4.4 2.2M12.5 1.5v3.7H8.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                        {move || if generating.get() { "Generating..." } else { "Regenerate" }}
                    </button>
                    // Export dropdown
                    <div style="position:relative">
                        <button
                            style="padding:5px 12px;font-size:12px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-2);cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                            on:click=move |_| export_open.update(|v| *v = !*v)
                        >
                            <svg width="11" height="11" viewBox="0 0 13 13" fill="none">
                                <path d="M6.5 1 L6.5 9 M3 6.5 L6.5 9.5 L10 6.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                                <path d="M1.5 10.5 L11.5 10.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                            </svg>
                            "Export PBC"
                            <svg width="9" height="9" viewBox="0 0 10 10" fill="none"><path d="M2 3.5 L5 6.5 L8 3.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
                        </button>
                        {move || export_open.get().then(|| view! {
                            <div style="position:absolute;top:calc(100% + 4px);right:0;background:var(--w-surface-1);border:0.5px solid var(--w-border-2);border-radius:6px;overflow:hidden;z-index:100;min-width:160px">
                                <div
                                    style="padding:8px 12px;font-size:12px;cursor:pointer;color:var(--w-text-1);border-bottom:0.5px solid var(--w-border)"
                                    on:click=move |_| {
                                        export_open.set(false);
                                        spawn_local(async move {
                                            status.set("Exporting Word document…".into());
                                            match tauri_invoke::<bool>("export_pbc_docx").await {
                                                Ok(true)  => status.set("PBC list exported as Word document.".into()),
                                                Ok(false) => status.set(String::new()),
                                                Err(e)    => status.set(format!("Export error: {e}")),
                                            }
                                        });
                                    }
                                >
                                    <span style="font-size:10px;font-weight:500;padding:2px 5px;border-radius:3px;background:#dbeafe;color:#1e40af;font-family:monospace;margin-right:8px">".docx"</span>
                                    "Word document"
                                </div>
                                <div
                                    style="padding:8px 12px;font-size:12px;cursor:pointer;color:var(--w-text-1)"
                                    on:click=move |_| {
                                        export_open.set(false);
                                        spawn_local(async move {
                                            status.set("Exporting PDF…".into());
                                            match tauri_invoke::<bool>("export_pbc_pdf").await {
                                                Ok(true)  => status.set("PBC list exported as PDF.".into()),
                                                Ok(false) => status.set(String::new()),
                                                Err(e)    => status.set(format!("Export error: {e}")),
                                            }
                                        });
                                    }
                                >
                                    <span style="font-size:10px;font-weight:500;padding:2px 5px;border-radius:3px;background:#fee2e2;color:#991b1b;font-family:monospace;margin-right:8px">".pdf"</span>
                                    "PDF"
                                </div>
                            </div>
                        })}
                    </div>
                </div>
            </div>

            // ── Scrollable body ───────────────────────────────────────────────
            <div style="flex:1;min-height:0;overflow-y:auto;padding:12px 14px;display:flex;flex-direction:column;gap:10px">

                // Stats row
                <div style="display:flex;gap:8px;flex-shrink:0">
                    <StatCard label="Data requests" value=Signal::derive(move || total_items().to_string()) green=false />
                    <StatCard label="Approved"      value=Signal::derive(move || approved_count().to_string()) green=true />
                    <StatCard label="SQL queries"   value=Signal::derive(move || sql_count().to_string()) green=false />
                    <StatCard label="CSV / uploads" value=Signal::derive(move || csv_count().to_string()) green=false />
                </div>

                // AI instruction box (hidden when list is approved)
                {move || (!list_approved.get()).then(|| view! {
                    <div style="border:0.5px solid var(--w-border-2);border-radius:6px;overflow:hidden;flex-shrink:0">
                        <div style="display:flex;align-items:center;gap:8px;padding:8px 12px;background:var(--w-surface-2);border-bottom:0.5px solid var(--w-border);font-size:12px;color:var(--w-text-3)">
                            <div style="width:16px;height:16px;border-radius:50%;background:#178856;display:flex;align-items:center;justify-content:center;flex-shrink:0">
                                <svg width="8" height="8" viewBox="0 0 10 10" fill="none"><circle cx="5" cy="5" r="3.5" stroke="#fff" stroke-width="1.2"/><circle cx="5" cy="5" r="1.2" fill="#fff"/></svg>
                            </div>
                            "Ask AI to add, remove, or refine data requests"
                        </div>
                        <div style="display:flex;align-items:center">
                            <textarea
                                style="flex:1;padding:9px 12px;font-size:12px;border:none;background:transparent;color:var(--w-text-1);font-family:var(--font);resize:none;outline:none"
                                rows="2"
                                placeholder="e.g. \"Add a request for the vehicle registry with VIN numbers\" or \"C-03 also needs fuel level at return\""
                                prop:value=move || ai_prompt.get()
                                prop:disabled=move || ai_loading.get()
                                on:input=move |ev| ai_prompt.set(event_target_value(&ev))
                            />
                            <button
                                style="margin:6px;padding:5px 12px;font-size:12px;border-radius:4px;border:none;background:#178856;color:#fff;cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:5px;white-space:nowrap"
                                prop:disabled=move || ai_loading.get()
                                on:click=move |_| {
                                    let prompt_text = ai_prompt.get();
                                    if prompt_text.trim().is_empty() || ai_loading.get() { return; }
                                    ai_loading.set(true);
                                    ai_status.set(None);
                                    let current = groups.get();
                                    let current_json = serde_json::to_string(
                                        &current.iter().map(|g| serde_json::json!({
                                            "controlRef": g.control_ref,
                                            "title": g.title,
                                            "items": g.items.iter().map(|i| serde_json::json!({
                                                "id": i.id, "name": i.name,
                                                "item_type": i.item_type,
                                                "table_name": i.table_name,
                                                "fields": i.fields,
                                            })).collect::<Vec<_>>()
                                        })).collect::<Vec<_>>()
                                    ).unwrap_or_default();
                                    let full_prompt = format!(
                                        "{REFINE_PBC_LIST}{current_json}\n\
                                         User instruction: {prompt_text}"
                                    );
                                    let audit_plan_snap = audit_plan.get_untracked();
                                    spawn_local(async move {
                                        match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &full_prompt).await {
                                            Err(e) => {
                                                ai_status.set(Some(format!("Error: {e}")));
                                                ai_loading.set(false);
                                            }
                                            Ok(raw) => {
                                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                                    if let Some(add_arr) = v["add_items"].as_array() {
                                                        for item_json in add_arr {
                                                            let cref = item_json["controlRef"].as_str().unwrap_or("");
                                                            let ctrl = audit_plan_snap.iter()
                                                                .flat_map(|p| p.controls.iter())
                                                                .find(|c| c.control_ref == cref);
                                                            if let Some(ctrl) = ctrl {
                                                                let fields: Vec<String> = item_json["fields"]
                                                                    .as_array()
                                                                    .map(|a| a.iter().filter_map(|f| f.as_str().map(|s| s.to_string())).collect())
                                                                    .unwrap_or_default();
                                                                let _ = tauri_invoke_args::<PbcItem>("save_pbc_item",
                                                                    serde_json::json!({
                                                                        "controlId":   ctrl.id,
                                                                        "controlRef":  cref,
                                                                        "name":        item_json["name"].as_str().unwrap_or(""),
                                                                        "itemType":    item_json["itemType"].as_str().unwrap_or("SQL"),
                                                                        "tableName":   item_json["tableName"].as_str(),
                                                                        "fields":      fields,
                                                                        "purpose":     item_json["purpose"].as_str().unwrap_or(""),
                                                                        "scopeFormat": item_json["scopeFormat"].as_str().unwrap_or("Audit period"),
                                                                    }),
                                                                ).await;
                                                            }
                                                        }
                                                    }
                                                    let current_groups = groups.get_untracked();
                                                    if let Some(add_fields_arr) = v["add_fields"].as_array() {
                                                        for change in add_fields_arr {
                                                            let item_id = change["itemId"].as_str().unwrap_or("");
                                                            let new_fields: Vec<String> = change["fields"].as_array()
                                                                .map(|a| a.iter().filter_map(|f| f.as_str().map(|s| s.to_string())).collect())
                                                                .unwrap_or_default();
                                                            if let Some(item) = current_groups.iter().flat_map(|g| g.items.iter()).find(|i| i.id == item_id) {
                                                                let mut merged = item.fields.clone();
                                                                for f in new_fields { if !merged.contains(&f) { merged.push(f); } }
                                                                let _ = tauri_invoke_args::<()>("update_pbc_item_fields",
                                                                    serde_json::json!({"itemId": item_id, "fields": merged})).await;
                                                            }
                                                        }
                                                    }
                                                    if let Some(rm_fields_arr) = v["remove_fields"].as_array() {
                                                        for change in rm_fields_arr {
                                                            let item_id = change["itemId"].as_str().unwrap_or("");
                                                            let rm: Vec<String> = change["fields"].as_array()
                                                                .map(|a| a.iter().filter_map(|f| f.as_str().map(|s| s.to_string())).collect())
                                                                .unwrap_or_default();
                                                            if let Some(item) = current_groups.iter().flat_map(|g| g.items.iter()).find(|i| i.id == item_id) {
                                                                let filtered: Vec<String> = item.fields.iter().filter(|f| !rm.contains(f)).cloned().collect();
                                                                let _ = tauri_invoke_args::<()>("update_pbc_item_fields",
                                                                    serde_json::json!({"itemId": item_id, "fields": filtered})).await;
                                                            }
                                                        }
                                                    }
                                                    let summary = v["summary"].as_str().unwrap_or("Changes applied.").to_string();
                                                    ai_status.set(Some(summary));
                                                } else {
                                                    ai_status.set(Some("Could not parse AI response.".into()));
                                                }
                                                reload_groups(groups, approved_ids).await;
                                                ai_prompt.set(String::new());
                                                ai_loading.set(false);
                                            }
                                        }
                                    });
                                }
                            >
                                {move || if ai_loading.get() {
                                    view! { <Spinner size=11 /> }.into_any()
                                } else {
                                    view! { <svg width="11" height="11" viewBox="0 0 12 12" fill="none"><path d="M1 6 L11 6 M7 2 L11 6 L7 10" stroke="#fff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                                }}
                                "Update"
                            </button>
                        </div>
                        {move || ai_status.get().map(|s| view! {
                            <div style="padding:6px 12px 8px;font-size:12px;color:#178856;border-top:0.5px solid var(--w-border)">{s}</div>
                        })}
                    </div>
                })}

                // ── Sync PBC → audit plan ─────────────────────────────────────
                {move || (!list_approved.get()).then(|| view! {
                    <div style="border:0.5px solid var(--w-border-2);border-radius:6px;overflow:hidden;flex-shrink:0">
                        <div style="display:flex;align-items:center;justify-content:space-between;padding:8px 12px;background:var(--w-surface-2);border-bottom:0.5px solid var(--w-border)">
                            <div style="display:flex;align-items:center;gap:8px;font-size:12px;color:var(--w-text-3)">
                                <div style="width:16px;height:16px;border-radius:50%;background:var(--w-border-2);display:flex;align-items:center;justify-content:center;flex-shrink:0">
                                    <svg width="8" height="8" viewBox="0 0 10 10" fill="none"><path d="M9 5A4 4 0 1 1 5 1c1.3 0 2.4.62 3.1 1.57M9 1v3H6" stroke="var(--w-text-3)" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
                                </div>
                                "Sync changes back to audit plan"
                            </div>
                            <button
                                style=move || format!(
                                    "padding:4px 12px;font-size:12px;border-radius:4px;border:none;background:{};color:#fff;cursor:{};font-family:var(--font);display:flex;align-items:center;gap:5px;white-space:nowrap",
                                    if sync_loading.get() { "var(--w-border-2)" } else { "#178856" },
                                    if sync_loading.get() { "default" } else { "pointer" },
                                )
                                prop:disabled=move || sync_loading.get() || generating.get()
                                on:click=move |_| {
                                    if sync_loading.get() || generating.get() { return; }
                                    sync_loading.set(true);
                                    sync_status.set(None);

                                    let plan = audit_plan.get_untracked();
                                    let current_groups = groups.get_untracked();

                                    // Build readable context: PBC grouped by control
                                    let mut pbc_text = String::new();
                                    for g in &current_groups {
                                        if g.items.is_empty() { continue; }
                                        pbc_text.push_str(&format!("Control {}: {}\n", g.control_ref, g.title));
                                        for item in &g.items {
                                            pbc_text.push_str(&format!(
                                                "  - {} [{}] table:{} fields:{} purpose:{} scope:{}\n",
                                                item.name, item.item_type,
                                                item.table_name.as_deref().unwrap_or("—"),
                                                item.fields.join(", "),
                                                item.purpose, item.scope_format
                                            ));
                                        }
                                    }

                                    // Build readable audit plan context
                                    let mut plan_text = String::new();
                                    for proc in &plan {
                                        plan_text.push_str(&format!("Process: {}\n", proc.process_name));
                                        for ctrl in &proc.controls {
                                            plan_text.push_str(&format!(
                                                "  Control {}: objective={} | procedure={} | risk={}\n",
                                                ctrl.control_ref, ctrl.control_objective,
                                                ctrl.test_procedure, ctrl.risk_level
                                            ));
                                        }
                                    }

                                    let prompt = format!(
                                        "{}\nAUDIT PLAN:\n{}\nPBC DATA REQUESTS:\n{}",
                                        vinrouge::audit_prompts::SYNC_PBC_TO_PLAN,
                                        plan_text, pbc_text
                                    );

                                    spawn_local(async move {
                                        match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                                            Err(e) => {
                                                sync_status.set(Some(format!("Error: {e}")));
                                                sync_loading.set(false);
                                            }
                                            Ok(raw) => {
                                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                                    if let Some(updates) = v["updates"].as_array() {
                                                        for upd in updates {
                                                            let cref  = upd["control_ref"].as_str().unwrap_or("");
                                                            let field = upd["field"].as_str().unwrap_or("");
                                                            let value = upd["value"].as_str().unwrap_or("");
                                                            if cref.is_empty() || field.is_empty() { continue; }

                                                            // Find control ID in the current plan
                                                            let ctrl_id = plan.iter()
                                                                .flat_map(|p| p.controls.iter())
                                                                .find(|c| c.control_ref == cref)
                                                                .map(|c| c.id.clone());

                                                            if let Some(id) = ctrl_id {
                                                                let _ = tauri_invoke_args::<()>("update_control_field",
                                                                    serde_json::json!({"controlId": id, "field": field, "value": value})
                                                                ).await;
                                                            }
                                                        }

                                                        // Reload audit_plan so step 2 sees the changes
                                                        if let Ok(updated) = tauri_invoke::<Vec<crate::types::AuditProcessWithControls>>("list_audit_plan").await {
                                                            audit_plan.set(updated);
                                                        }

                                                        let summary = v["summary"].as_str().unwrap_or("Audit plan updated.").to_string();
                                                        sync_status.set(Some(summary));
                                                    } else {
                                                        sync_status.set(Some("No updates suggested.".into()));
                                                    }
                                                } else {
                                                    sync_status.set(Some("Could not parse AI response.".into()));
                                                }
                                                sync_loading.set(false);
                                            }
                                        }
                                    });
                                }
                            >
                                {move || if sync_loading.get() {
                                    view! { <Spinner size=11 /> }.into_any()
                                } else {
                                    view! { <svg width="11" height="11" viewBox="0 0 14 14" fill="none"><path d="M12.5 7A5.5 5.5 0 1 1 7 1.5c1.8 0 3.4.87 4.4 2.2M12.5 1.5v3.7H8.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                                }}
                                {move || if sync_loading.get() { "Syncing..." } else { "Sync to plan" }}
                            </button>
                        </div>
                        {move || sync_status.get().map(|s| view! {
                            <div style="padding:6px 12px 8px;font-size:12px;color:#178856">{s}</div>
                        })}
                    </div>
                })}

                // Generating spinner
                {move || generating.get().then(|| view! {
                    <div style="text-align:center;padding:32px;font-size:13px;color:var(--w-text-3);display:flex;flex-direction:column;align-items:center;gap:10px">
                        <svg width="20" height="20" viewBox="0 0 14 14" fill="none" style="animation:spin 1s linear infinite">
                            <circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.4" stroke-dasharray="22" stroke-dashoffset="8" stroke-linecap="round"/>
                        </svg>
                        {move || {
                            let phase = gen_phase.get();
                            if phase.is_empty() { "Generating data requests from audit plan...".to_string() }
                            else { phase }
                        }}
                    </div>
                })}

                // Error banner
                {move || gen_error.get().map(|e| view! {
                    <div style="padding:10px 14px;background:rgba(239,68,68,0.08);border:0.5px solid rgba(239,68,68,0.3);border-radius:6px;font-size:12px;color:#ef4444">
                        <strong>"Generation failed: "</strong>{e}
                    </div>
                })}

                // PBC groups — only groups that have items
                {move || {
                    groups.get().into_iter()
                        .filter(|g| !g.items.is_empty())
                        .map(|g| view! {
                            <PbcGroupCard
                                group=g
                                approved_ids=approved_ids
                                list_approved=list_approved
                                groups=groups
                                approved_ids_reload=approved_ids
                            />
                        }).collect_view()
                }}

            </div>

            // ── Approval bar ──────────────────────────────────────────────────
            <div style="flex-shrink:0;padding:10px 14px;border-top:1px solid var(--w-border);background:var(--w-surface-2);display:flex;align-items:center;gap:12px">
                <button
                    style="padding:6px 14px;font-size:12px;border-radius:4px;border:0.5px solid var(--w-border-2);background:transparent;color:var(--w-text-2);cursor:pointer;font-family:var(--font);display:flex;align-items:center;gap:6px"
                    on:click=move |_| audit_ui_step.set(2)
                >
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                        <path d="M10 6 L2 6 M5.5 2.5 L2 6 L5.5 9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    "Back"
                </button>

                <ProgressRing
                    approved=Signal::derive(move || approved_count())
                    total=Signal::derive(move || total_items())
                />

                <div
                    style="display:flex;align-items:center;gap:8px;cursor:pointer"
                    on:click=move |_| {
                        if generating.get() { return; }
                        let new_val = !list_approved.get();
                        list_approved.set(new_val);
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("set_pbc_list_approved",
                                serde_json::json!({"approved": new_val})).await;
                        });
                    }
                >
                    <div style=move || format!(
                        "width:17px;height:17px;border-radius:4px;border:1.5px solid {};background:{};display:flex;align-items:center;justify-content:center;flex-shrink:0",
                        if list_approved.get() { "#178856" } else { "var(--w-border-2)" },
                        if list_approved.get() { "#178856" } else { "transparent" },
                    )>
                        {move || list_approved.get().then(|| view! {
                            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                                <polyline points="1.5,5 4,7.5 8.5,2.5" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                            </svg>
                        })}
                    </div>
                    <span style="font-size:12px;color:var(--w-text-2);white-space:nowrap">"I have reviewed and approve this PBC list"</span>
                </div>

                <button
                    style=move || format!(
                        "margin-left:auto;padding:6px 16px;font-size:12px;border-radius:4px;border:none;background:{};color:#fff;cursor:{};font-family:var(--font);display:flex;align-items:center;gap:6px",
                        if list_approved.get() { "#178856" } else { "var(--w-border-2)" },
                        if list_approved.get() { "pointer" } else { "not-allowed" },
                    )
                    prop:disabled=move || !list_approved.get()
                    on:click=move |_| { if list_approved.get() { audit_ui_step.set(4); } }
                >
                    "Proceed to data collection"
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                        <path d="M2 6 L10 6 M6.5 2.5 L10 6 L6.5 9.5" stroke="#fff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </button>
            </div>
        </div>
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn do_generate(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    groups: RwSignal<Vec<PbcGroup>>,
    approved_ids: RwSignal<Vec<String>>,
    gen_phase: RwSignal<String>,
    gen_error: RwSignal<Option<String>>,
    generating: RwSignal<bool>,
) {
    // Always reload from DB so inline edits made in Step 2 are reflected.
    let plan = match crate::ipc::tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
        Ok(p) => { audit_plan.set(p.clone()); p }
        Err(e) => {
            gen_error.set(Some(format!("Failed to load audit plan: {e}")));
            generating.set(false);
            return;
        }
    };

    if plan.is_empty() {
        gen_error.set(Some("No audit plan found — complete Step 2 first.".into()));
        generating.set(false);
        return;
    }

    // Format the plan as clean readable text — raw JSON wastes tokens on IDs and timestamps
    gen_phase.set(format!("Preparing {} processes for Ollama...", plan.len()));
    let mut plan_text = String::new();
    for process in &plan {
        plan_text.push_str(&format!("Process: {}\n", process.process_name));
        plan_text.push_str(&format!("Description: {}\n", process.description));
        for ctrl in &process.controls {
            plan_text.push_str(&format!(
                "  Control {}: {}\n",
                ctrl.control_ref, ctrl.control_objective
            ));
            plan_text.push_str(&format!(
                "    How it operates: {}\n",
                ctrl.control_description
            ));
            plan_text.push_str(&format!("    Test procedure: {}\n", ctrl.test_procedure));
            plan_text.push_str(&format!("    Risk: {}\n", ctrl.risk_level));
        }
        plan_text.push('\n');
    }

    let prompt = format!("{}\n\n{}", vinrouge::audit_prompts::GENERATE_PBC, plan_text);

    gen_phase.set("Asking Ollama to generate data requests (may take a minute)...".into());
    match ask_ollama_structured(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt, crate::step1::prompts::pbc_list_schema()).await {
        Err(e) => {
            gen_error.set(Some(format!("{e}")));
            gen_phase.set(String::new());
        }
        Ok(raw) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&raw);
            let v = match parsed {
                Ok(v) => v,
                Err(e) => {
                    let preview: String = raw.chars().take(300).collect();
                    gen_error.set(Some(format!("JSON parse error: {e}\nResponse preview: {preview}")));
                    gen_phase.set(String::new());
                    generating.set(false);
                    return;
                }
            };
            let arr = match v["items"].as_array() {
                Some(a) => a.clone(),
                None => {
                    let preview: String = raw.chars().take(300).collect();
                    gen_error.set(Some(format!("Response missing 'items' array.\nResponse preview: {preview}")));
                    gen_phase.set(String::new());
                    generating.set(false);
                    return;
                }
            };
            let mut saved = 0usize;
            let mut unmatched: Vec<String> = Vec::new();
            for item_json in &arr {
                // Accept both snake_case and camelCase keys from models
                let cref = item_json["control_ref"]
                    .as_str()
                    .or_else(|| item_json["controlRef"].as_str())
                    .unwrap_or("");
                let ctrl = plan
                    .iter()
                    .flat_map(|p| p.controls.iter())
                    .find(|c| c.control_ref == cref);
                match ctrl {
                    None => { unmatched.push(cref.to_string()); }
                    Some(ctrl) => {
                        let fields: Vec<String> = item_json["fields"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|f| f.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let item_type = item_json["item_type"]
                            .as_str()
                            .or_else(|| item_json["itemType"].as_str())
                            .or_else(|| item_json["type"].as_str())
                            .unwrap_or("SQL");
                        let table_name = item_json["table_name"]
                            .as_str()
                            .or_else(|| item_json["tableName"].as_str());
                        let scope_format = item_json["scope_format"]
                            .as_str()
                            .or_else(|| item_json["scopeFormat"].as_str())
                            .unwrap_or("Audit period");
                        match tauri_invoke_args::<PbcItem>(
                            "save_pbc_item",
                            serde_json::json!({
                                "controlId":   ctrl.id,
                                "controlRef":  cref,
                                "name":        item_json["name"].as_str().unwrap_or(""),
                                "itemType":    item_type,
                                "tableName":   table_name,
                                "fields":      fields,
                                "purpose":     item_json["purpose"].as_str().unwrap_or(""),
                                "scopeFormat": scope_format,
                            }),
                        )
                        .await {
                            Ok(_) => { saved += 1; }
                            Err(e) => { unmatched.push(format!("{cref} (save error: {e})")); }
                        }
                    }
                }
            }
            if saved == 0 {
                let detail = if unmatched.is_empty() {
                    "No items in response.".to_string()
                } else {
                    format!("Unmatched/failed control refs: {}", unmatched.join(", "))
                };
                gen_error.set(Some(format!("Nothing saved. {detail}")));
                gen_phase.set(String::new());
                generating.set(false);
                return;
            }
            if !unmatched.is_empty() {
                gen_error.set(Some(format!(
                    "Saved {saved} items. Could not match: {}",
                    unmatched.join(", ")
                )));
            }
            gen_phase.set("Loading results...".into());
            reload_groups(groups, approved_ids).await;
            gen_phase.set(String::new());
        }
    }
    generating.set(false);
}

async fn reload_groups(groups: RwSignal<Vec<PbcGroup>>, approved_ids: RwSignal<Vec<String>>) {
    if let Ok(g) =
        tauri_invoke_args::<Vec<PbcGroup>>("list_pbc_groups", serde_json::json!({})).await
    {
        let ids: Vec<String> = g
            .iter()
            .flat_map(|grp| grp.items.iter())
            .filter(|i| i.approved)
            .map(|i| i.id.clone())
            .collect();
        approved_ids.set(ids);
        groups.set(g);
    }
}

// ── PbcGroupCard ──────────────────────────────────────────────────────────────

#[component]
fn PbcGroupCard(
    group: PbcGroup,
    approved_ids: RwSignal<Vec<String>>,
    list_approved: RwSignal<bool>,
    groups: RwSignal<Vec<PbcGroup>>,
    approved_ids_reload: RwSignal<Vec<String>>,
) -> impl IntoView {
    let open: RwSignal<bool> = RwSignal::new(true);
    let control_id = group.control_id.clone();
    let control_ref = group.control_ref.clone();
    let title = group.title.clone();
    let process_name = group.process_name.clone();
    let item_count = RwSignal::new(group.items.len());
    let items = RwSignal::new(group.items.clone());

    let ai_prompt: RwSignal<String> = RwSignal::new(String::new());
    let ai_loading: RwSignal<bool> = RwSignal::new(false);
    let ai_status: RwSignal<Option<String>> = RwSignal::new(None);

    let group_item_ids: Vec<String> = group.items.iter().map(|i| i.id.clone()).collect();
    let approved_in_group = {
        let ids = group_item_ids.clone();
        Signal::derive(move || {
            let approved = approved_ids.get();
            ids.iter().filter(|id| approved.contains(id)).count()
        })
    };

    // Extra clones for closures inside the reactive `then(|| view! {...})` block.
    // The header view consumes control_ref / title / process_name as static text,
    // so we must clone them beforehand for use in on:click handlers.
    let add_ctrl_id = control_id.clone();
    let add_ctrl_ref = control_ref.clone();
    let ai_ctrl_id = control_id.clone();
    let ai_ctrl_ref = control_ref.clone();
    let ai_title_str = title.clone();

    view! {
        <div style="border:0.5px solid var(--w-border);border-radius:6px;overflow:visible">
            // ── Card header ───────────────────────────────────────────────────
            <div
                style=move || format!(
                    "display:flex;align-items:center;gap:10px;padding:9px 12px;background:var(--w-surface-2);cursor:pointer;user-select:none;border-radius:{};{}",
                    if open.get() { "6px 6px 0 0" } else { "6px" },
                    if open.get() { "border-bottom:0.5px solid var(--w-border)" } else { "" }
                )
                on:click=move |_| open.update(|v| *v = !*v)
            >
                <svg
                    style=move || format!(
                        "flex-shrink:0;color:var(--w-text-3);transition:transform 0.15s;transform:{}",
                        if open.get() { "rotate(90deg)" } else { "rotate(0deg)" }
                    )
                    width="11" height="11" viewBox="0 0 14 14" fill="none"
                >
                    <polyline points="4,3 10,7 4,11" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <div style="flex:1;min-width:0">
                    <div style="font-size:12.5px;font-weight:500;color:var(--w-text-1);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.3">
                        {title}
                    </div>
                    <div style="display:flex;align-items:center;gap:4px;margin-top:1px">
                        <span style="font-size:10.5px;font-family:monospace;color:var(--w-text-3)">{control_ref}</span>
                        <span style="font-size:10px;color:var(--w-border-2)">"·"</span>
                        <span style="font-size:10.5px;color:var(--w-text-3);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{process_name}</span>
                    </div>
                </div>
                <span style=move || format!(
                    "font-size:11px;padding:2px 8px;border-radius:999px;flex-shrink:0;border:0.5px solid {};background:{};color:{}",
                    if approved_in_group.get() == item_count.get() && item_count.get() > 0 { "#178856" } else { "var(--w-border)" },
                    if approved_in_group.get() == item_count.get() && item_count.get() > 0 { "rgba(23,136,86,0.12)" } else { "var(--w-surface-1)" },
                    if approved_in_group.get() == item_count.get() && item_count.get() > 0 { "#178856" } else { "var(--w-text-3)" },
                )>
                    {move || format!("{}/{}", approved_in_group.get(), item_count.get())}
                </span>
            </div>

            // ── Table — only when open ────────────────────────────────────────
            {move || open.get().then(|| view! {
                <div style="overflow-x:auto;border-radius:0 0 6px 6px">
                    <table class="pbc-table">
                        <thead>
                            <tr>
                                <th>"#"</th>
                                <th class="pbc-name-cell">"Request name"</th>
                                <th class="pbc-type-cell">"Type / source"</th>
                                <th>"Fields required"</th>
                                <th style="width:20%">"Purpose"</th>
                                <th class="pbc-scope-cell center">"Scope"</th>
                                <th class="pbc-ok-cell center">"OK"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || items.get().into_iter().enumerate().map(|(idx, item)| {
                                let item_id     = item.id.clone();
                                let approved_id = item.id.clone();
                                let approved    = Signal::derive(move || approved_ids.get().contains(&approved_id));

                                // ── Editable signals ──────────────────────
                                let e_name    = RwSignal::new(item.name.clone());
                                let e_type    = RwSignal::new(item.item_type.clone());
                                let e_table   = RwSignal::new(item.table_name.clone().unwrap_or_default());
                                let e_fields  = RwSignal::new(item.fields.join(", "));
                                let e_purpose = RwSignal::new(item.purpose.clone());
                                let e_scope   = RwSignal::new(item.scope_format.clone());

                                // Save all fields to the backend
                                let save = {
                                    let id = item_id.clone();
                                    move || {
                                        let id       = id.clone();
                                        let name     = e_name.get_untracked();
                                        let typ      = e_type.get_untracked();
                                        let tbl      = e_table.get_untracked();
                                        let fields: Vec<String> = e_fields.get_untracked()
                                            .split(',').map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty()).collect();
                                        let purpose  = e_purpose.get_untracked();
                                        let scope    = e_scope.get_untracked();
                                        spawn_local(async move {
                                            let _ = tauri_invoke_args::<()>("update_pbc_item", serde_json::json!({
                                                "itemId":      id,
                                                "name":        name,
                                                "itemType":    typ,
                                                "tableName":   if tbl.is_empty() { serde_json::Value::Null } else { tbl.into() },
                                                "fields":      fields,
                                                "purpose":     purpose,
                                                "scopeFormat": scope,
                                            })).await;
                                        });
                                    }
                                };
                                let save2 = save.clone();
                                let save3 = save.clone();
                                let save4 = save.clone();
                                let save5 = save.clone();
                                let save6 = save.clone();

                                // Delete item
                                let del_id = item_id.clone();
                                let on_delete = move |_| {
                                    let id = del_id.clone();
                                    spawn_local(async move {
                                        let _ = tauri_invoke_args::<()>("delete_pbc_item", serde_json::json!({"itemId": id})).await;
                                        reload_groups(groups, approved_ids_reload).await;
                                    });
                                };

                                // Toggle approved
                                let tog_id  = item_id.clone();
                                let tog_id2 = item_id.clone();
                                let on_toggle = move |_| {
                                    if list_approved.get() { return; }
                                    let id  = tog_id.clone();
                                    let id2 = tog_id2.clone();
                                    spawn_local(async move {
                                        if let Ok(new_state) = tauri_invoke_args::<bool>(
                                            "toggle_pbc_item_approved",
                                            serde_json::json!({"itemId": id}),
                                        ).await {
                                            approved_ids.update(|ids| {
                                                if new_state {
                                                    if !ids.contains(&id2) { ids.push(id2.clone()); }
                                                } else {
                                                    ids.retain(|x| x != &id2);
                                                }
                                            });
                                        }
                                    });
                                };

                                view! {
                                    <tr class:approved=move || approved.get()>
                                        // # ────────────────────────────────────
                                        <td class="pbc-num-cell">
                                            {idx + 1}
                                        </td>

                                        // Request name ────────────────────────
                                        <td class="pbc-name-cell">
                                            <input
                                                class="pbc-cell-input"
                                                prop:value=move || e_name.get()
                                                on:input=move |ev| e_name.set(event_target_value(&ev))
                                                on:blur=move |_| save()
                                            />
                                        </td>

                                        // Type / source ───────────────────────
                                        <td class="pbc-type-cell">
                                            <div class="pbc-type-inner">
                                                <select
                                                    class="pbc-type-select"
                                                    on:change=move |ev| {
                                                        e_type.set(event_target_value(&ev));
                                                        save2();
                                                    }
                                                >
                                                    <option value="SQL" selected=move || e_type.get() == "SQL">"SQL"</option>
                                                    <option value="CSV" selected=move || e_type.get() == "CSV">"CSV"</option>
                                                    <option value="Manual" selected=move || e_type.get() == "Manual">"Manual"</option>
                                                </select>
                                                <input
                                                    class="pbc-source-input"
                                                    placeholder="table / source…"
                                                    prop:value=move || e_table.get()
                                                    on:input=move |ev| e_table.set(event_target_value(&ev))
                                                    on:blur=move |_| save3()
                                                />
                                            </div>
                                        </td>

                                        // Fields required ─────────────────────
                                        <td>
                                            <input
                                                class="pbc-fields-input"
                                                placeholder="field1, field2…"
                                                prop:value=move || e_fields.get()
                                                on:input=move |ev| e_fields.set(event_target_value(&ev))
                                                on:blur=move |_| save4()
                                            />
                                        </td>

                                        // Purpose ─────────────────────────────
                                        <td>
                                            <input
                                                class="pbc-cell-input-sm"
                                                prop:value=move || e_purpose.get()
                                                on:input=move |ev| e_purpose.set(event_target_value(&ev))
                                                on:blur=move |_| save5()
                                            />
                                        </td>

                                        // Scope ───────────────────────────────
                                        <td class="pbc-scope-cell">
                                            <input
                                                class="pbc-scope-input"
                                                prop:value=move || e_scope.get()
                                                on:input=move |ev| e_scope.set(event_target_value(&ev))
                                                on:blur=move |_| save6()
                                            />
                                        </td>

                                        // OK + delete ─────────────────────────
                                        <td class="pbc-ok-cell">
                                            <div class="pbc-ok-inner">
                                                <button
                                                    class="pbc-ok-btn"
                                                    class:checked=move || approved.get()
                                                    prop:disabled=move || list_approved.get()
                                                    on:click=on_toggle
                                                >
                                                    <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
                                                        <polyline points="1.5,6 4.5,9.5 10.5,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                                                    </svg>
                                                </button>
                                                <button
                                                    class="pbc-del-btn"
                                                    title="Delete row"
                                                    on:click=on_delete
                                                >
                                                    <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
                                                        <line x1="2" y1="2" x2="10" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                                                        <line x1="10" y1="2" x2="2" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                                                    </svg>
                                                </button>
                                            </div>
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>

                // ── Add request ───────────────────────────────────────────────
                <div style="padding:6px 10px 4px">
                    <DashedAddButton label=" Add request" on_click={
                        let cid  = add_ctrl_id.clone();
                        let cref = add_ctrl_ref.clone();
                        move || {
                            let cid  = cid.clone();
                            let cref = cref.clone();
                            spawn_local(async move {
                                if let Ok(item) = tauri_invoke_args::<PbcItem>("save_pbc_item", serde_json::json!({
                                    "controlId":   cid,
                                    "controlRef":  cref,
                                    "name":        "New request",
                                    "itemType":    "SQL",
                                    "tableName":   serde_json::Value::Null,
                                    "fields":      serde_json::json!([]),
                                    "purpose":     "",
                                    "scopeFormat": "Audit period",
                                })).await {
                                    items.update(|v| v.push(item));
                                    item_count.update(|n| *n += 1);
                                }
                            });
                        }
                    } />
                </div>

                // ── Per-group AI instruction ──────────────────────────────────
                <SectionPrompt
                    prompt=ai_prompt
                    loading=ai_loading
                    status=ai_status
                    placeholder="e.g. \"Add a request for driver licence scans\" or \"Remove the fuel level field\""
                    on_send={
                        let ctrl_ref_str = ai_ctrl_ref.clone();
                        let ctrl_title   = ai_title_str.clone();
                        let ctrl_cid     = ai_ctrl_id.clone();
                        let ctrl_cref    = ai_ctrl_ref.clone();
                        move || {
                            let instruction = ai_prompt.get();
                            if instruction.trim().is_empty() || ai_loading.get() { return; }
                            ai_loading.set(true);
                            ai_status.set(None);

                            let current_items = items.get_untracked();
                            let items_json = serde_json::to_string(
                                &current_items.iter().map(|i| serde_json::json!({
                                    "id": i.id, "name": i.name,
                                    "item_type": i.item_type,
                                    "table_name": i.table_name,
                                    "fields": i.fields,
                                    "purpose": i.purpose,
                                    "scope_format": i.scope_format,
                                })).collect::<Vec<_>>()
                            ).unwrap_or_default();

                            let ctrl_ref_str = ctrl_ref_str.clone();
                            let ctrl_title   = ctrl_title.clone();
                            let ctrl_cid     = ctrl_cid.clone();
                            let ctrl_cref    = ctrl_cref.clone();

                            let prompt = format!(
                                "{}\nControl: {ctrl_ref_str} — {ctrl_title}\n\
                                 Current items: {items_json}\n\
                                 User instruction: {instruction}",
                                vinrouge::audit_prompts::UPDATE_PBC_GROUP
                            );

                            spawn_local(async move {
                                match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                                    Err(e) => {
                                        ai_status.set(Some(format!("Error: {e}")));
                                        ai_loading.set(false);
                                    }
                                    Ok(raw) => {
                                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                            if let Some(arr) = v["add_items"].as_array() {
                                                for item_json in arr {
                                                    let fields: Vec<String> = item_json["fields"].as_array()
                                                        .map(|a| a.iter().filter_map(|f| f.as_str().map(|s| s.to_string())).collect())
                                                        .unwrap_or_default();
                                                    let tbl = item_json["tableName"].as_str()
                                                        .or_else(|| item_json["table_name"].as_str());
                                                    let _ = tauri_invoke_args::<PbcItem>("save_pbc_item", serde_json::json!({
                                                        "controlId":   ctrl_cid,
                                                        "controlRef":  ctrl_cref,
                                                        "name":        item_json["name"].as_str().unwrap_or(""),
                                                        "itemType":    item_json["itemType"].as_str().or_else(|| item_json["item_type"].as_str()).unwrap_or("SQL"),
                                                        "tableName":   tbl,
                                                        "fields":      fields,
                                                        "purpose":     item_json["purpose"].as_str().unwrap_or(""),
                                                        "scopeFormat": item_json["scopeFormat"].as_str().or_else(|| item_json["scope_format"].as_str()).unwrap_or("Audit period"),
                                                    })).await;
                                                }
                                            }
                                            if let Some(arr) = v["remove_item_ids"].as_array() {
                                                for id_val in arr {
                                                    if let Some(id) = id_val.as_str() {
                                                        let _ = tauri_invoke_args::<()>("delete_pbc_item",
                                                            serde_json::json!({"itemId": id})).await;
                                                    }
                                                }
                                            }
                                            let summary = v["summary"].as_str().unwrap_or("Done.").to_string();
                                            ai_status.set(Some(summary));
                                        } else {
                                            ai_status.set(Some("Could not parse AI response.".into()));
                                        }
                                        reload_groups(groups, approved_ids_reload).await;
                                        ai_prompt.set(String::new());
                                        ai_loading.set(false);
                                    }
                                }
                            });
                        }
                    }
                />
            })}
        </div>
    }
}
