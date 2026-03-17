use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::types::{AuditProcessWithControls, PbcGroup, PbcItem};
use crate::ipc::tauri_invoke_args;
use crate::ollama::{OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, ask_ollama_json};

// ── Step3View ─────────────────────────────────────────────────────────────────

#[component]
pub fn Step3View(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let groups: RwSignal<Vec<PbcGroup>>         = RwSignal::new(vec![]);
    // Separate signal for approval state so toggling doesn't re-render group cards
    let approved_ids: RwSignal<Vec<String>>     = RwSignal::new(vec![]);
    let list_approved: RwSignal<bool>           = RwSignal::new(false);
    let generating: RwSignal<bool>              = RwSignal::new(false);
    let ai_prompt: RwSignal<String>             = RwSignal::new(String::new());
    let ai_loading: RwSignal<bool>              = RwSignal::new(false);
    let ai_status: RwSignal<Option<String>>     = RwSignal::new(None);
    let export_open: RwSignal<bool>             = RwSignal::new(false);

    // ── Generation helper ─────────────────────────────────────────────────────
    let run_generate = move || {
        generating.set(true);
        groups.set(vec![]);
        approved_ids.set(vec![]);
        let plan = audit_plan.get_untracked();
        let plan_json = serde_json::to_string(&plan).unwrap_or_default();
        let prompt = format!("{}\n\n{}", vinrouge::audit_prompts::GENERATE_PBC, plan_json);
        spawn_local(async move {
            match ask_ollama_json(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt).await {
                Err(e) => {
                    status.set(format!("PBC generation error: {e}"));
                    generating.set(false);
                }
                Ok(raw) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                        if let Some(arr) = v["items"].as_array() {
                            let plan_snap = audit_plan.get_untracked();
                            for item_json in arr {
                                let cref = item_json["control_ref"].as_str().unwrap_or("");
                                let ctrl = plan_snap.iter()
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
                                            "itemType":    item_json["item_type"].as_str().unwrap_or("SQL"),
                                            "tableName":   item_json["table_name"].as_str(),
                                            "fields":      fields,
                                            "purpose":     item_json["purpose"].as_str().unwrap_or(""),
                                            "scopeFormat": item_json["scope_format"].as_str().unwrap_or("Audit period"),
                                        }),
                                    ).await;
                                }
                            }
                        }
                    }
                    reload_groups(groups, approved_ids).await;
                    generating.set(false);
                }
            }
        });
    };

    // ── Initial load ──────────────────────────────────────────────────────────
    spawn_local(async move {
        if let Ok(v) = tauri_invoke_args::<bool>("get_pbc_list_approved", serde_json::json!({})).await {
            list_approved.set(v);
        }
        match tauri_invoke_args::<Vec<PbcGroup>>("list_pbc_groups", serde_json::json!({})).await {
            Ok(g) if g.iter().any(|grp| !grp.items.is_empty()) => {
                let ids: Vec<String> = g.iter()
                    .flat_map(|grp| grp.items.iter())
                    .filter(|i| i.approved)
                    .map(|i| i.id.clone())
                    .collect();
                approved_ids.set(ids);
                groups.set(g);
            }
            _ => run_generate(),
        }
    });

    // ── Derived stats ─────────────────────────────────────────────────────────
    let total_items    = move || groups.get().iter().flat_map(|g| g.items.iter()).count();
    let approved_count = move || approved_ids.get().len();
    let sql_count      = move || groups.get().iter().flat_map(|g| g.items.iter()).filter(|i| i.item_type == "SQL").count();
    let csv_count      = move || groups.get().iter().flat_map(|g| g.items.iter()).filter(|i| i.item_type == "CSV").count();

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

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
                            spawn_local(async move {
                                let _ = tauri_invoke_args::<()>("clear_pbc_items", serde_json::json!({})).await;
                                let _ = tauri_invoke_args::<()>("set_pbc_list_approved", serde_json::json!({"approved": false})).await;
                                list_approved.set(false);
                            });
                            run_generate();
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
                                        status.set("Word export coming soon.".into());
                                    }
                                >
                                    <span style="font-size:10px;font-weight:500;padding:2px 5px;border-radius:3px;background:#dbeafe;color:#1e40af;font-family:monospace;margin-right:8px">".docx"</span>
                                    "Word document"
                                </div>
                                <div
                                    style="padding:8px 12px;font-size:12px;cursor:pointer;color:var(--w-text-1)"
                                    on:click=move |_| {
                                        export_open.set(false);
                                        status.set("PDF export coming soon.".into());
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
            <div style="flex:1;overflow-y:auto;padding:12px 14px;display:flex;flex-direction:column;gap:10px">

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
                                        "You are an audit data analyst. Update a PBC list based on the user instruction.\n\
                                         Return ONLY JSON, no markdown:\n\
                                         {{\"summary\":\"one sentence\",\
                                         \"add_items\":[{{\"controlRef\":\"C-01\",\"name\":\"\",\"itemType\":\"SQL\",\"tableName\":null,\"fields\":[],\"purpose\":\"\",\"scopeFormat\":\"\"}}],\
                                         \"add_fields\":[{{\"itemId\":\"...\",\"fields\":[\"f1\"]}}],\
                                         \"remove_fields\":[{{\"itemId\":\"...\",\"fields\":[\"f1\"]}}]}}\n\
                                         Only include keys where changes are needed.\n\
                                         Current PBC list: {current_json}\n\
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
                                    view! { <svg width="11" height="11" viewBox="0 0 14 14" fill="none" style="animation:spin 1s linear infinite"><circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.4" stroke-dasharray="22" stroke-dashoffset="8" stroke-linecap="round"/></svg> }.into_any()
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

                // Generating spinner
                {move || generating.get().then(|| view! {
                    <div style="text-align:center;padding:32px;font-size:13px;color:var(--w-text-3);display:flex;flex-direction:column;align-items:center;gap:10px">
                        <svg width="20" height="20" viewBox="0 0 14 14" fill="none" style="animation:spin 1s linear infinite">
                            <circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.4" stroke-dasharray="22" stroke-dashoffset="8" stroke-linecap="round"/>
                        </svg>
                        "Generating data requests from audit plan..."
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

async fn reload_groups(groups: RwSignal<Vec<PbcGroup>>, approved_ids: RwSignal<Vec<String>>) {
    if let Ok(g) = tauri_invoke_args::<Vec<PbcGroup>>("list_pbc_groups", serde_json::json!({})).await {
        let ids: Vec<String> = g.iter()
            .flat_map(|grp| grp.items.iter())
            .filter(|i| i.approved)
            .map(|i| i.id.clone())
            .collect();
        approved_ids.set(ids);
        groups.set(g);
    }
}

// ── StatCard ──────────────────────────────────────────────────────────────────

#[component]
fn StatCard(label: &'static str, value: Signal<String>, green: bool) -> impl IntoView {
    view! {
        <div style="flex:1;padding:10px 12px;background:var(--w-surface-2);border-radius:6px;border:0.5px solid var(--w-border);min-width:0">
            <div style=move || format!(
                "font-size:18px;font-weight:500;line-height:1;margin-bottom:3px;color:{}",
                if green { "#178856" } else { "var(--w-text-1)" }
            )>
                {move || value.get()}
            </div>
            <div style="font-size:11px;color:var(--w-text-3)">{label}</div>
        </div>
    }
}

// ── ProgressRing ──────────────────────────────────────────────────────────────

#[component]
fn ProgressRing(approved: Signal<usize>, total: Signal<usize>) -> impl IntoView {
    let pct = move || {
        let t = total.get();
        if t == 0 { 0.0f64 } else { (approved.get() as f64 / t as f64) * 100.0 }
    };
    view! {
        <div style="display:flex;align-items:center;gap:8px;flex-shrink:0">
            <svg width="34" height="34" viewBox="0 0 36 36">
                <circle cx="18" cy="18" r="14" fill="none" stroke="var(--w-border)" stroke-width="3"/>
                <circle cx="18" cy="18" r="14" fill="none" stroke="#178856" stroke-width="3"
                    stroke-dasharray="87.96 87.96"
                    stroke-dashoffset=move || format!("{:.2}", 87.96 * (1.0 - pct() / 100.0))
                    stroke-linecap="round"
                    transform="rotate(-90 18 18)"/>
                <text x="18" y="22" text-anchor="middle" font-size="9" fill="#178856"
                    font-family="var(--font)" font-weight="500">
                    {move || format!("{:.0}%", pct())}
                </text>
            </svg>
            <span style="font-size:12px;color:var(--w-text-3);white-space:nowrap">
                {move || format!("{} of {}", approved.get(), total.get())}
            </span>
        </div>
    }
}

// ── PbcGroupCard ──────────────────────────────────────────────────────────────

#[component]
fn PbcGroupCard(
    group: PbcGroup,
    approved_ids: RwSignal<Vec<String>>,
    list_approved: RwSignal<bool>,
) -> impl IntoView {
    let open: RwSignal<bool> = RwSignal::new(true);
    let control_ref          = group.control_ref.clone();
    let title                = group.title.clone();
    let process_name         = group.process_name.clone();
    let item_count           = group.items.len();
    let items                = group.items.clone();

    view! {
        <div style="border:0.5px solid var(--w-border);border-radius:6px;overflow:hidden">
            // Header
            <div
                style=move || format!(
                    "display:flex;align-items:center;gap:10px;padding:9px 12px;background:var(--w-surface-2);cursor:pointer;user-select:none;{}",
                    if open.get() { "border-bottom:0.5px solid var(--w-border)" } else { "" }
                )
                on:click=move |_| open.update(|v| *v = !*v)
            >
                <svg
                    style=move || format!(
                        "flex-shrink:0;color:var(--w-text-3);transition:transform 0.15s;transform:{}",
                        if open.get() { "rotate(90deg)" } else { "rotate(0deg)" }
                    )
                    width="12" height="12" viewBox="0 0 14 14" fill="none"
                >
                    <polyline points="4,3 10,7 4,11" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <div style="flex:1;min-width:0">
                    <div style="display:flex;align-items:center;gap:8px">
                        <span style="font-size:13px;font-weight:500;color:var(--w-text-1)">{title}</span>
                        <span style="font-size:11px;color:var(--w-text-3);font-family:monospace">{control_ref}</span>
                    </div>
                    <div style="font-size:11px;color:var(--w-text-3);margin-top:2px">{process_name}</div>
                </div>
                <span style="font-size:11px;padding:2px 8px;border-radius:999px;background:var(--w-surface-1);color:var(--w-text-3);border:0.5px solid var(--w-border)">
                    {item_count} " request" {if item_count != 1 { "s" } else { "" }}
                </span>
            </div>

            // Table — only when open
            {move || open.get().then(|| {
                let rows = items.clone();
                view! {
                    <div style="overflow-x:auto">
                        <table style="width:100%;border-collapse:collapse;font-size:12px;table-layout:fixed">
                            <thead>
                                <tr style="background:var(--w-surface-2)">
                                    <th style="padding:6px 10px;text-align:left;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:72px">"ID"</th>
                                    <th style="padding:6px 10px;text-align:left;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:18%">"Request name"</th>
                                    <th style="padding:6px 10px;text-align:left;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:15%">"Type / source"</th>
                                    <th style="padding:6px 10px;text-align:left;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase">"Fields required"</th>
                                    <th style="padding:6px 10px;text-align:left;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:18%">"Purpose"</th>
                                    <th style="padding:6px 10px;text-align:center;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:90px">"Scope"</th>
                                    <th style="padding:6px 10px;text-align:center;font-weight:500;color:var(--w-text-3);font-size:11px;border-bottom:0.5px solid var(--w-border);letter-spacing:0.04em;text-transform:uppercase;width:46px">"OK"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rows.into_iter().enumerate().map(|(idx, item)| {
                                    let item_id   = item.id.clone();
                                    let item_id2  = item.id.clone();
                                    let is_last   = idx == item_count - 1;
                                    // Derive approved state from parent signal — no groups re-render needed
                                    let approved_id = item.id.clone();
                                    let approved = Signal::derive(move || approved_ids.get().contains(&approved_id));
                                    view! {
                                        <tr style=move || format!(
                                            "{}{}",
                                            if !is_last { "border-bottom:0.5px solid var(--w-border);" } else { "" },
                                            if approved.get() { "opacity:0.6" } else { "" },
                                        )>
                                            <td style=move || format!(
                                                "padding:8px 10px;vertical-align:top;font-family:monospace;font-size:11px;color:var(--w-text-3);{}",
                                                if approved.get() { "border-left:2px solid #178856" } else { "" },
                                            )>
                                                {format!("PBC-{:03}", idx + 1)}
                                            </td>
                                            <td style="padding:8px 10px;vertical-align:top;font-weight:500;color:var(--w-text-1)">{item.name.clone()}</td>
                                            <td style="padding:8px 10px;vertical-align:top">
                                                <span style=format!(
                                                    "display:inline-block;padding:2px 7px;border-radius:4px;font-size:11px;font-weight:500;{}",
                                                    if item.item_type == "SQL" {
                                                        "background:rgba(219,234,254,0.25);color:#3b82f6"
                                                    } else {
                                                        "background:rgba(254,243,199,0.25);color:#d97706"
                                                    }
                                                )>{item.item_type.clone()}</span>
                                                {item.table_name.clone().map(|t| view! {
                                                    <div style="font-size:10.5px;font-family:monospace;color:var(--w-text-3);margin-top:3px">{t}</div>
                                                })}
                                            </td>
                                            <td style="padding:8px 10px;vertical-align:top">
                                                <div style="display:flex;flex-wrap:wrap;gap:3px">
                                                    {item.fields.iter().map(|f| view! {
                                                        <span style="display:inline-block;padding:1px 5px;border-radius:3px;background:var(--w-surface-2);border:0.5px solid var(--w-border);font-size:10.5px;font-family:monospace;color:var(--w-text-3)">{f.clone()}</span>
                                                    }).collect_view()}
                                                </div>
                                            </td>
                                            <td style="padding:8px 10px;vertical-align:top;font-size:11.5px;color:var(--w-text-3)">{item.purpose.clone()}</td>
                                            <td style="padding:8px 10px;vertical-align:top;font-size:11px;color:var(--w-text-3);text-align:center">{item.scope_format.clone()}</td>
                                            <td style="padding:8px 10px;vertical-align:middle;text-align:center">
                                                <button
                                                    style=move || format!(
                                                        "width:24px;height:24px;border-radius:4px;border:0.5px solid {};background:{};cursor:{};display:flex;align-items:center;justify-content:center;color:{};margin:auto",
                                                        if approved.get() { "#178856" } else { "var(--w-border-2)" },
                                                        if approved.get() { "#178856" } else { "transparent" },
                                                        if list_approved.get() { "default" } else { "pointer" },
                                                        if approved.get() { "#fff" } else { "var(--w-text-3)" },
                                                    )
                                                    prop:disabled=move || list_approved.get()
                                                    on:click=move |_| {
                                                        if list_approved.get() { return; }
                                                        let id  = item_id.clone();
                                                        let id2 = item_id2.clone();
                                                        spawn_local(async move {
                                                            if let Ok(new_state) = tauri_invoke_args::<bool>(
                                                                "toggle_pbc_item_approved",
                                                                serde_json::json!({"itemId": id}),
                                                            ).await {
                                                                // Update only approved_ids — groups signal untouched
                                                                approved_ids.update(|ids| {
                                                                    if new_state {
                                                                        if !ids.contains(&id2) { ids.push(id2.clone()); }
                                                                    } else {
                                                                        ids.retain(|x| x != &id2);
                                                                    }
                                                                });
                                                            }
                                                        });
                                                    }
                                                >
                                                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                                                        <polyline points="1.5,6 4.5,9.5 10.5,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                                                    </svg>
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }
            })}
        </div>
    }
}
