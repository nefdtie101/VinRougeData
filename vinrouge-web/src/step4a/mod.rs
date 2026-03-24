use std::collections::HashMap;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, Spinner};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_ollama_structured, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, PbcGroup, SessionSchema};
use vinrouge::audit_prompts::{dsl_script_schema, GENERATE_DSL};

// ── Script review state ───────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
enum ScriptStatus {
    Generated,
    Edited,
    Approved,
    Rejected,
}

#[derive(Clone, Debug)]
struct ScriptState {
    status: ScriptStatus,
    text: String,
}

// ── Phase ─────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum Phase {
    Loading,
    Generating,
    Review,
    Running { done: usize, total: usize },
    Error(String),
}

// ── DSL operation parsing ─────────────────────────────────────────────────────

fn parse_ops(text: &str) -> Vec<(&'static str, &'static str)> {
    // Returns (keyword_label, css-suffix) pairs, deduped by css-suffix
    let kws: &[(&'static str, &'static str)] = &[
        ("TOTAL", "total"),
        ("SUM", "total"),
        ("COUNT", "count"),
        ("AVERAGE", "avg"),
        ("AVG", "avg"),
        ("PERCENT", "pct"),
        ("EXPECT", "expect"),
        ("ASSERT", "expect"),
        ("FLAG", "flag"),
        ("MATCH", "match"),
        ("SAMPLE", "match"),
    ];
    let mut seen_cls: Vec<&'static str> = vec![];
    let mut ops: Vec<(&'static str, &'static str)> = vec![];
    for (kw, cls) in kws {
        if text.contains(kw) && !seen_cls.contains(cls) {
            seen_cls.push(cls);
            ops.push((*kw, *cls));
        }
    }
    ops
}

// ── Step4aView ────────────────────────────────────────────────────────────────

#[component]
pub fn Step4aView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let phase: RwSignal<Phase> = RwSignal::new(Phase::Loading);
    let schemas: RwSignal<Vec<SessionSchema>> = RwSignal::new(vec![]);
    let scripts: RwSignal<Vec<DslScript>> = RwSignal::new(vec![]);
    let script_states: RwSignal<HashMap<String, ScriptState>> = RwSignal::new(HashMap::new());
    let expanded_groups: RwSignal<HashMap<String, bool>> = RwSignal::new(HashMap::new());
    let progress_msg: RwSignal<String> = RwSignal::new("Loading data…".to_string());

    // Auto-start generation on mount
    spawn_local(async move {
        do_generate(
            audit_plan,
            phase,
            schemas,
            scripts,
            script_states,
            expanded_groups,
            progress_msg,
            status,
        )
        .await;
    });

    // ── Derived stats ─────────────────────────────────────────────────────────
    let total_count = move || scripts.get().len();
    let approved_count = move || {
        script_states
            .get()
            .values()
            .filter(|s| s.status == ScriptStatus::Approved)
            .count()
    };
    let pending_count = move || {
        let ss = script_states.get();
        let decided = ss
            .values()
            .filter(|s| {
                matches!(s.status, ScriptStatus::Approved | ScriptStatus::Rejected)
            })
            .count();
        scripts.get().len().saturating_sub(decided)
    };

    let can_run = move || {
        if !matches!(phase.get(), Phase::Review) {
            return false;
        }
        let ss = script_states.get();
        let rejected = ss
            .values()
            .filter(|s| s.status == ScriptStatus::Rejected)
            .count();
        !scripts.get().is_empty() && scripts.get().len() > rejected
    };

    // ── Generate all handler ──────────────────────────────────────────────────
    let on_generate_all = move |_| {
        spawn_local(async move {
            do_generate(
                audit_plan,
                phase,
                schemas,
                scripts,
                script_states,
                expanded_groups,
                progress_msg,
                status,
            )
            .await;
        });
    };

    // ── Run engine handler ────────────────────────────────────────────────────
    let on_run = move |_| {
        if !can_run() {
            return;
        }
        let ss = script_states.get_untracked();
        let all_scripts = scripts.get_untracked();
        let has_any_approved = ss.values().any(|s| s.status == ScriptStatus::Approved);
        let to_run: Vec<DslScript> = all_scripts
            .into_iter()
            .filter(|s| {
                ss.get(&s.id)
                    .map(|st| {
                        if has_any_approved {
                            st.status == ScriptStatus::Approved
                        } else {
                            st.status != ScriptStatus::Rejected
                        }
                    })
                    .unwrap_or(true)
            })
            .collect();
        if to_run.is_empty() {
            return;
        }
        let total = to_run.len();
        spawn_local(async move {
            for (i, script) in to_run.iter().enumerate() {
                phase.set(Phase::Running { done: i, total });
                // Persist any edits before running
                if let Some(st) = ss.get(&script.id) {
                    if !matches!(st.status, ScriptStatus::Generated) {
                        let _ = tauri_invoke_args::<()>(
                            "update_dsl_script",
                            serde_json::json!({
                                "scriptId": script.id,
                                "scriptText": st.text,
                            }),
                        )
                        .await;
                    }
                }
                let _ = tauri_invoke_args::<Vec<serde_json::Value>>(
                    "run_dsl_script",
                    serde_json::json!({ "scriptId": script.id }),
                )
                .await;
            }
            audit_ui_step.set(6);
        });
    };

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Page header ────────────────────────────────────────────────────
            <div class="s4a-page-hdr">
                <div class="s4a-hdr-left">
                    <span class="s4a-page-step">"Step 4a"</span>
                    <span class="s4a-page-title">"Algorithm review"</span>
                    <span class="s4a-page-sub">
                        "Review and approve each DSL algorithm before the engine runs"
                    </span>
                </div>
                <div class="s4a-page-stats">
                    <div class="s4a-stat-item">
                        <span class="s4a-stat-num">{move || total_count()}</span>
                        <span class="s4a-stat-label">"Generated"</span>
                    </div>
                    <div class="s4a-stat-divider"></div>
                    <div class="s4a-stat-item">
                        <span class="s4a-stat-num s4a-stat-green">{move || approved_count()}</span>
                        <span class="s4a-stat-label">"Approved"</span>
                    </div>
                    <div class="s4a-stat-divider"></div>
                    <div class="s4a-stat-item">
                        <span class="s4a-stat-num">{move || pending_count()}</span>
                        <span class="s4a-stat-label">"Pending"</span>
                    </div>
                    <div class="s4a-stat-divider"></div>
                    <button
                        class="s4a-btn-gen-all"
                        disabled=Signal::derive(move || {
                            !matches!(phase.get(), Phase::Review | Phase::Error(_))
                        })
                        on:click=on_generate_all
                    >
                        "↻  Generate all"
                    </button>
                </div>
            </div>

            // ── Content ────────────────────────────────────────────────────────
            <div style="flex:1;overflow-y:auto;padding:14px 14px 4px">

                // Progress / error
                {move || match phase.get() {
                    Phase::Loading | Phase::Generating | Phase::Running { .. } => {
                        Some(view! {
                            <div class="s4-uploading" style="margin-bottom:12px;gap:10px">
                                <Spinner size=14 />
                                <span>{move || progress_msg.get()}</span>
                            </div>
                        }
                        .into_any())
                    }
                    Phase::Error(msg) => Some(view! {
                        <div style="margin-bottom:12px">
                            <Banner
                                message=Signal::derive(move || msg.clone())
                                variant="error"
                            />
                        </div>
                    }
                    .into_any()),
                    Phase::Review => None,
                }}

                // Control group cards
                {move || {
                    if !matches!(phase.get(), Phase::Review) {
                        return None;
                    }
                    let all_scripts = scripts.get();
                    if all_scripts.is_empty() {
                        return None;
                    }

                    // Build control title lookup from audit plan
                    let plan = audit_plan.get();
                    let ctrl_titles: HashMap<String, String> = plan
                        .iter()
                        .flat_map(|p| p.controls.iter())
                        .map(|c| (c.control_ref.clone(), c.control_objective.clone()))
                        .collect();

                    // Group scripts by control_ref, preserving order
                    let mut group_order: Vec<String> = vec![];
                    let mut grouped: HashMap<String, Vec<DslScript>> = HashMap::new();
                    for s in &all_scripts {
                        if !grouped.contains_key(&s.control_ref) {
                            group_order.push(s.control_ref.clone());
                        }
                        grouped
                            .entry(s.control_ref.clone())
                            .or_default()
                            .push(s.clone());
                    }

                    let group_views = group_order
                        .into_iter()
                        .map(|ctrl_ref| {
                            let items = grouped.remove(&ctrl_ref).unwrap_or_default();
                            let item_count = items.len();
                            let title = ctrl_titles
                                .get(&ctrl_ref)
                                .cloned()
                                .unwrap_or_else(|| ctrl_ref.clone());

                            // Clone refs for each closure that needs them
                            let cr_toggle    = ctrl_ref.clone();
                            let cr_hdr_cls   = ctrl_ref.clone();
                            let cr_chev_cls  = ctrl_ref.clone();
                            let cr_prog_cls  = ctrl_ref.clone();
                            let cr_prog_txt  = ctrl_ref.clone();
                            let cr_body      = ctrl_ref.clone();
                            let cr_display   = ctrl_ref.clone();
                            let items_for_rows = items.clone();
                            // IDs used for approved count (avoids cloning whole Vec twice)
                            let item_ids_cls: Vec<String> = items.iter().map(|s| s.id.clone()).collect();
                            let item_ids_txt: Vec<String> = items.iter().map(|s| s.id.clone()).collect();

                            let toggle_group = move |_| {
                                expanded_groups.update(|m| {
                                    let cur = m.get(&cr_toggle).copied().unwrap_or(true);
                                    m.insert(cr_toggle.clone(), !cur);
                                });
                            };

                            view! {
                                <div class="s4a-group-wrap">
                                    // ── Group header ──────────────────────────
                                    <div
                                        class=move || {
                                            if expanded_groups.get().get(&cr_hdr_cls).copied().unwrap_or(true) {
                                                "s4a-group-hdr s4a-group-hdr-open"
                                            } else {
                                                "s4a-group-hdr"
                                            }
                                        }
                                        on:click=toggle_group
                                    >
                                        <span class=move || {
                                            if expanded_groups.get().get(&cr_chev_cls).copied().unwrap_or(true) {
                                                "s4a-group-chev s4a-chev-open"
                                            } else {
                                                "s4a-group-chev"
                                            }
                                        }>
                                            "▸"
                                        </span>
                                        <div class="s4a-group-hdr-text">
                                            <div class="s4a-group-title">{title}</div>
                                            <span class="s4a-meta-ref">{cr_display}</span>
                                        </div>
                                        <span class=move || {
                                            let ss = script_states.get();
                                            let a = item_ids_cls.iter().filter(|id| ss.get(*id).map(|st| st.status == ScriptStatus::Approved).unwrap_or(false)).count();
                                            if a == item_count && item_count > 0 {
                                                "s4a-group-progress s4a-progress-done"
                                            } else {
                                                "s4a-group-progress"
                                            }
                                        }>
                                            {move || {
                                                let ss = script_states.get();
                                                let a = item_ids_txt.iter().filter(|id| ss.get(*id).map(|st| st.status == ScriptStatus::Approved).unwrap_or(false)).count();
                                                format!("{a}/{item_count}")
                                            }}
                                        </span>
                                    </div>

                                    // ── Group body (collapsible) ───────────────
                                    {move || {
                                        expanded_groups.get().get(&cr_body).copied().unwrap_or(true).then(|| {
                                            let row_views = items_for_rows
                                                .iter()
                                                .enumerate()
                                                .map(|(idx, script)| {
                                                    // Per-row signal IDs
                                                    let sid_row     = script.id.clone();
                                                    let sid_ta      = script.id.clone();
                                                    let sid_text    = script.id.clone();
                                                    let sid_ops     = script.id.clone();
                                                    let sid_badge   = script.id.clone();
                                                    let sid_aprchk  = script.id.clone();
                                                    let sid_approve = script.id.clone();
                                                    let sid_reject  = script.id.clone();
                                                    let sid_input   = script.id.clone();
                                                    let label       = script.label.clone();

                                                    // Row highlight
                                                    let row_cls = move || {
                                                        match script_states
                                                            .get()
                                                            .get(&sid_row)
                                                            .map(|s| s.status.clone())
                                                        {
                                                            Some(ScriptStatus::Approved) => {
                                                                "s4a-tr-approved"
                                                            }
                                                            Some(ScriptStatus::Rejected) => {
                                                                "s4a-tr-rejected"
                                                            }
                                                            _ => "",
                                                        }
                                                    };

                                                    // Textarea class (amber tint when edited)
                                                    let ta_cls = move || {
                                                        match script_states
                                                            .get()
                                                            .get(&sid_ta)
                                                            .map(|s| s.status.clone())
                                                        {
                                                            Some(ScriptStatus::Edited) => {
                                                                "s4a-dsl-ta s4a-ta-edited"
                                                            }
                                                            _ => "s4a-dsl-ta",
                                                        }
                                                    };

                                                    // Current text (reactive)
                                                    let current_text = move || {
                                                        script_states
                                                            .get()
                                                            .get(&sid_text)
                                                            .map(|s| s.text.clone())
                                                            .unwrap_or_default()
                                                    };

                                                    // Operations pills
                                                    let ops_view = move || {
                                                        let text = script_states
                                                            .get()
                                                            .get(&sid_ops)
                                                            .map(|s| s.text.clone())
                                                            .unwrap_or_default();
                                                        parse_ops(&text)
                                                            .into_iter()
                                                            .map(|(kw, cls_suf)| {
                                                                let cls = format!(
                                                                    "s4a-op-pill s4a-op-{cls_suf}"
                                                                );
                                                                view! {
                                                                    <span class=cls>{kw}</span>
                                                                }
                                                            })
                                                            .collect_view()
                                                    };

                                                    // Status badge
                                                    let badge_view = move || {
                                                        let (cls, lbl) = match script_states
                                                            .get()
                                                            .get(&sid_badge)
                                                            .map(|s| s.status.clone())
                                                            .unwrap_or(ScriptStatus::Generated)
                                                        {
                                                            ScriptStatus::Generated => (
                                                                "s4a-badge s4a-badge-generated",
                                                                "Generated",
                                                            ),
                                                            ScriptStatus::Edited => (
                                                                "s4a-badge s4a-badge-edited",
                                                                "Edited",
                                                            ),
                                                            ScriptStatus::Approved => (
                                                                "s4a-badge s4a-badge-approved",
                                                                "Approved",
                                                            ),
                                                            ScriptStatus::Rejected => (
                                                                "s4a-badge s4a-badge-rejected",
                                                                "Rejected",
                                                            ),
                                                        };
                                                        view! { <span class=cls>{lbl}</span> }
                                                    };

                                                    // Approve button class (filled when active)
                                                    let approve_btn_cls = move || {
                                                        if script_states
                                                            .get()
                                                            .get(&sid_aprchk)
                                                            .map(|s| s.status == ScriptStatus::Approved)
                                                            .unwrap_or(false)
                                                        {
                                                            "s4a-ok-btn s4a-ok-checked"
                                                        } else {
                                                            "s4a-ok-btn"
                                                        }
                                                    };

                                                    // Input handler — updates text + sets Edited
                                                    let on_input = move |ev: web_sys::Event| {
                                                        use wasm_bindgen::JsCast;
                                                        if let Some(ta) = ev
                                                            .target()
                                                            .and_then(|t| {
                                                                t.dyn_into::<
                                                                    web_sys::HtmlTextAreaElement,
                                                                >()
                                                                .ok()
                                                            })
                                                        {
                                                            let val = ta.value();
                                                            let sid = sid_input.clone();
                                                            script_states.update(|map| {
                                                                if let Some(s) = map.get_mut(&sid) {
                                                                    s.text = val;
                                                                    if s.status
                                                                        == ScriptStatus::Generated
                                                                    {
                                                                        s.status =
                                                                            ScriptStatus::Edited;
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    };

                                                    // Approve toggle
                                                    let on_approve = move |_| {
                                                        let sid = sid_approve.clone();
                                                        script_states.update(|map| {
                                                            if let Some(s) = map.get_mut(&sid) {
                                                                s.status = if s.status
                                                                    == ScriptStatus::Approved
                                                                {
                                                                    ScriptStatus::Generated
                                                                } else {
                                                                    ScriptStatus::Approved
                                                                };
                                                            }
                                                        });
                                                    };

                                                    // Reject toggle
                                                    let on_reject = move |_| {
                                                        let sid = sid_reject.clone();
                                                        script_states.update(|map| {
                                                            if let Some(s) = map.get_mut(&sid) {
                                                                s.status = if s.status
                                                                    == ScriptStatus::Rejected
                                                                {
                                                                    ScriptStatus::Generated
                                                                } else {
                                                                    ScriptStatus::Rejected
                                                                };
                                                            }
                                                        });
                                                    };

                                                    view! {
                                                        <tr class=row_cls>
                                                            <td class="s4a-cell-num">
                                                                {idx + 1}
                                                            </td>
                                                            <td>
                                                                <div class="s4a-cell-pad">
                                                                    <div class="s4a-pbc-name">
                                                                        {label}
                                                                    </div>
                                                                </div>
                                                            </td>
                                                            <td style="padding:0">
                                                                <textarea
                                                                    class=ta_cls
                                                                    prop:value=current_text
                                                                    on:input=on_input
                                                                ></textarea>
                                                            </td>
                                                            <td class="s4a-ops-cell">
                                                                {ops_view}
                                                            </td>
                                                            <td class="s4a-status-cell">
                                                                {badge_view}
                                                            </td>
                                                            <td class="s4a-ok-cell">
                                                                <div class="s4a-ok-inner">
                                                                    <button
                                                                        class=approve_btn_cls
                                                                        on:click=on_approve
                                                                        title="Approve"
                                                                    >
                                                                        "✓"
                                                                    </button>
                                                                    <button
                                                                        class="s4a-del-btn"
                                                                        on:click=on_reject
                                                                        title="Reject"
                                                                    >
                                                                        "✗"
                                                                    </button>
                                                                </div>
                                                            </td>
                                                        </tr>
                                                    }
                                                })
                                                .collect_view();

                                            view! {
                                                <div class="s4a-group-body">
                                                    <div style="overflow-x:auto">
                                                        <table class="s4a-dsl-table">
                                                            <colgroup>
                                                                <col class="s4a-col-num" />
                                                                <col class="s4a-col-pbc" />
                                                                <col class="s4a-col-dsl" />
                                                                <col class="s4a-col-ops" />
                                                                <col class="s4a-col-status" />
                                                                <col class="s4a-col-ok" />
                                                            </colgroup>
                                                            <thead>
                                                                <tr>
                                                                    <th>"#"</th>
                                                                    <th>"PBC request"</th>
                                                                    <th>"DSL algorithm"</th>
                                                                    <th>"Operations"</th>
                                                                    <th>"Status"</th>
                                                                    <th class="s4a-th-center">
                                                                        "OK"
                                                                    </th>
                                                                </tr>
                                                            </thead>
                                                            <tbody>{row_views}</tbody>
                                                        </table>
                                                    </div>
                                                </div>
                                            }
                                        })
                                    }}
                                </div>
                            }
                        })
                        .collect_view();

                    Some(view! { <div>{group_views}</div> }.into_any())
                }}
            </div>

            // ── Status bar ─────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || {
                    if can_run() { "s4-dot s4-dot--ready" } else { "s4-dot s4-dot--idle" }
                }></span>
                <span class="s4-status-text">
                    {move || match phase.get() {
                        Phase::Loading => "Loading data…".to_string(),
                        Phase::Generating => progress_msg.get(),
                        Phase::Running { done, total } => {
                            format!("Running scripts: {done}/{total}…")
                        }
                        Phase::Review => {
                            let a = approved_count();
                            let n = total_count();
                            if a == 0 {
                                format!("0 of {n} algorithms approved — review before running")
                            } else {
                                format!("{a} of {n} approved")
                            }
                        }
                        Phase::Error(e) => format!("Error: {e}"),
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton
                        label="Back"
                        back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(4))
                    />
                    <button
                        class=move || {
                            if can_run() {
                                "s4a-btn-run s4a-btn-run-ready"
                            } else {
                                "s4a-btn-run"
                            }
                        }
                        on:click=on_run
                    >
                        "▶  Run engine"
                    </button>
                </div>
            </div>

        </div>
    }
}

// ── Generation pipeline ───────────────────────────────────────────────────────

async fn do_generate(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    phase: RwSignal<Phase>,
    schemas: RwSignal<Vec<SessionSchema>>,
    scripts: RwSignal<Vec<DslScript>>,
    script_states: RwSignal<HashMap<String, ScriptState>>,
    expanded_groups: RwSignal<HashMap<String, bool>>,
    progress_msg: RwSignal<String>,
    status: RwSignal<String>,
) {
    phase.set(Phase::Loading);
    progress_msg.set("Loading imported data…".to_string());

    // 1. Load session schemas
    let session_schemas: Vec<SessionSchema> =
        match tauri_invoke("get_session_schemas").await {
            Ok(s) => s,
            Err(e) => {
                phase.set(Phase::Error(format!("Could not load data: {e}")));
                return;
            }
        };
    if session_schemas.is_empty() {
        phase.set(Phase::Error(
            "No data imported yet. Go back to Step 4 and upload files.".to_string(),
        ));
        return;
    }
    schemas.set(session_schemas.clone());

    // 2. Load PBC context
    let pbc_groups: Vec<PbcGroup> =
        tauri_invoke("list_pbc_groups").await.unwrap_or_default();

    // 3. Clear previous scripts
    let _ = tauri_invoke::<()>("clear_dsl_scripts").await;

    // 4. Generate via AI
    phase.set(Phase::Generating);
    progress_msg.set("Generating DSL algorithms via AI…".to_string());

    let schema_section = build_schema_section(&session_schemas);
    let plan_section = build_plan_section(&audit_plan.get_untracked(), &pbc_groups);
    let prompt = format!(
        "{GENERATE_DSL}\
         AVAILABLE DATA:\n{schema_section}\n\
         AUDIT CONTROLS TO TEST:\n{plan_section}\n\
         Return ONLY a JSON object: \
         {{\"scripts\": [{{\"control_ref\": \"C-1\", \"label\": \"Test label\", \
         \"script\": \"DSL code here\"}}]}}"
    );

    let schema = dsl_script_schema();
    let raw =
        match ask_ollama_structured(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt, schema)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                phase.set(Phase::Error(format!("AI generation failed: {e}")));
                return;
            }
        };

    // 5. Parse JSON response
    let script_dtos: Vec<(String, String, String)> = {
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                phase.set(Phase::Error(format!("Invalid JSON from AI: {e}")));
                return;
            }
        };
        v["scripts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let control_ref = item["control_ref"].as_str()?.to_string();
                let label = item["label"]
                    .as_str()
                    .unwrap_or(&control_ref)
                    .to_string();
                let script = item["script"].as_str()?.to_string();
                if script.trim().is_empty() {
                    return None;
                }
                Some((control_ref, label, script))
            })
            .collect()
    };

    if script_dtos.is_empty() {
        phase.set(Phase::Error(
            "AI did not generate any valid DSL scripts.".to_string(),
        ));
        return;
    }

    // 6. Save scripts to DB, build frontend state
    let plan = audit_plan.get_untracked();
    let ctrl_lookup: HashMap<String, String> = plan
        .iter()
        .flat_map(|p| p.controls.iter())
        .map(|c| (c.control_ref.clone(), c.id.clone()))
        .collect();

    let mut saved: Vec<DslScript> = vec![];
    let mut new_states: HashMap<String, ScriptState> = HashMap::new();
    let mut new_expanded: HashMap<String, bool> = HashMap::new();

    for (control_ref, label, script_text) in &script_dtos {
        let control_id = ctrl_lookup.get(control_ref).cloned().unwrap_or_default();
        match tauri_invoke_args::<DslScript>(
            "save_dsl_script",
            serde_json::json!({
                "controlId": control_id,
                "controlRef": control_ref,
                "label": label,
                "scriptText": script_text,
            }),
        )
        .await
        {
            Ok(s) => {
                new_states.insert(
                    s.id.clone(),
                    ScriptState {
                        status: ScriptStatus::Generated,
                        text: script_text.clone(),
                    },
                );
                new_expanded.insert(control_ref.clone(), true);
                saved.push(s);
            }
            Err(e) => {
                status.set(format!(
                    "Warning: could not save script for {control_ref}: {e}"
                ));
            }
        }
    }

    scripts.set(saved);
    script_states.set(new_states);
    expanded_groups.set(new_expanded);
    phase.set(Phase::Review);
}

// ── Prompt builders ───────────────────────────────────────────────────────────

fn build_schema_section(schemas: &[SessionSchema]) -> String {
    schemas
        .iter()
        .map(|s| {
            format!(
                "Table: {} ({} rows)\n  Columns: {}\n\n",
                s.table_name,
                s.row_count,
                s.columns.join(", ")
            )
        })
        .collect()
}

fn build_plan_section(plan: &[AuditProcessWithControls], pbc_groups: &[PbcGroup]) -> String {
    let mut s = String::new();
    for process in plan {
        s.push_str(&format!("Process: {}\n", process.process_name));
        for ctrl in &process.controls {
            s.push_str(&format!(
                "  Control {}: {}\n    Test: {}\n    Risk: {}\n",
                ctrl.control_ref,
                ctrl.control_objective,
                ctrl.test_procedure,
                ctrl.risk_level
            ));
            let pbc_items: Vec<String> = pbc_groups
                .iter()
                .filter(|g| g.control_id == ctrl.id)
                .flat_map(|g| g.items.iter())
                .map(|i| {
                    if let Some(tn) = &i.table_name {
                        format!("{} (table: {}, fields: {})", i.name, tn, i.fields.join(", "))
                    } else {
                        format!("{} (fields: {})", i.name, i.fields.join(", "))
                    }
                })
                .collect();
            if !pbc_items.is_empty() {
                s.push_str(&format!("    Data: {}\n", pbc_items.join("; ")));
            }
        }
        s.push('\n');
    }
    s
}
