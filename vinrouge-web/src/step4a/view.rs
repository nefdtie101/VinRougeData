use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, Spinner};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::types::{AuditProcessWithControls, JoinSpec, RelCandidate, SessionSchema};
use super::types::Phase;

#[component]
pub fn Step4aView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let _ = audit_plan;
    let _ = status;

    // Start immediately in Idle so the screen renders without waiting for DB
    let phase: RwSignal<Phase>                     = RwSignal::new(Phase::Idle);
    let schemas: RwSignal<Vec<SessionSchema>>       = RwSignal::new(vec![]);
    let join_candidates: RwSignal<Vec<RelCandidate>> = RwSignal::new(vec![]);
    let accepted_joins: RwSignal<Vec<bool>>         = RwSignal::new(vec![]);

    let manual_left_table: RwSignal<String>  = RwSignal::new(String::new());
    let manual_left_col: RwSignal<String>    = RwSignal::new(String::new());
    let manual_right_table: RwSignal<String> = RwSignal::new(String::new());
    let manual_right_col: RwSignal<String>   = RwSignal::new(String::new());

    // ── Load schemas in the background, polling until imports are ready ──────
    spawn_local(async move {
        // Poll up to 30 times (≈30 s) waiting for the import to finish.
        let mut attempts = 0u32;
        loop {
            let session_schemas: Vec<SessionSchema> =
                match tauri_invoke("get_session_schemas").await {
                    Ok(s) => s,
                    Err(e) => {
                        phase.set(Phase::Error(format!("Could not load data: {e}")));
                        return;
                    }
                };

            if session_schemas.is_empty() {
                attempts += 1;
                if attempts >= 30 {
                    phase.set(Phase::Error(
                        "No data imported. Go back to Step 4 and upload files.".to_string(),
                    ));
                    return;
                }
                // Still importing — show a gentle status and wait 1 s
                phase.set(Phase::Loading);
                gloo_timers::future::TimeoutFuture::new(1_000).await;
                phase.set(Phase::Idle);
                continue;
            }

            // If master already exists or only one dataset: skip straight to 4b
            let has_master = session_schemas.iter().any(|s| s.source_type == "master");
            let non_master = session_schemas.iter().filter(|s| s.source_type != "master").count();
            if has_master || non_master < 2 {
                audit_ui_step.set(6);
                return;
            }

            schemas.set(session_schemas);
            // phase stays Idle — screen is already visible
            break;
        }
    });

    // ── Detect relationships (triggered by button) ────────────────────────────
    let on_detect = move |_| {
        phase.set(Phase::Loading);
        spawn_local(async move {
            match tauri_invoke::<Vec<RelCandidate>>("detect_data_relationships").await {
                Ok(candidates) => {
                    let n = candidates.len();
                    join_candidates.set(candidates);
                    accepted_joins.set(vec![false; n]);
                }
                Err(_) => {
                    // Detection failed or found nothing — still show review panel
                    // so user can add manual joins
                    join_candidates.set(vec![]);
                    accepted_joins.set(vec![]);
                }
            }
            phase.set(Phase::RelationshipReview);
        });
    };

    // ── Add manual join ───────────────────────────────────────────────────────
    let on_add_manual_join = move |_| {
        let lt = manual_left_table.get_untracked();
        let lc = manual_left_col.get_untracked();
        let rt = manual_right_table.get_untracked();
        let rc = manual_right_col.get_untracked();
        if lt.is_empty() || lc.is_empty() || rt.is_empty() || rc.is_empty() || lt == rt {
            return;
        }
        let schemas_snap = schemas.get_untracked();
        let left_s  = schemas_snap.iter().find(|s| s.table_name == lt).cloned();
        let right_s = schemas_snap.iter().find(|s| s.table_name == rt).cloned();
        if let (Some(l), Some(r)) = (left_s, right_s) {
            let candidate = RelCandidate {
                left_import_id:  l.import_id,
                left_table:      lt,
                left_col:        lc,
                right_import_id: r.import_id,
                right_table:     rt,
                right_col:       rc,
                confidence:      100,
                overlap_count:   0,
            };
            join_candidates.update(|v| v.push(candidate));
            accepted_joins.update(|v| v.push(true));
            manual_left_col.set(String::new());
            manual_right_col.set(String::new());
        }
    };

    // ── Build master record then advance to 4b ────────────────────────────────
    let on_build_master = move |_| {
        let candidates = join_candidates.get_untracked();
        let accepted   = accepted_joins.get_untracked();
        let specs: Vec<JoinSpec> = candidates
            .into_iter()
            .zip(accepted.into_iter())
            .filter(|(_, ok)| *ok)
            .map(|(c, _)| JoinSpec {
                left_import_id:  c.left_import_id,
                left_col:        c.left_col,
                right_import_id: c.right_import_id,
                right_col:       c.right_col,
            })
            .collect();

        if specs.is_empty() {
            phase.set(Phase::Error("No joins selected — tick at least one.".to_string()));
            return;
        }

        phase.set(Phase::BuildingMaster);
        spawn_local(async move {
            match tauri_invoke_args::<String>(
                "build_master_record",
                serde_json::json!({ "joins": specs }),
            ).await {
                Ok(_)  => audit_ui_step.set(6),
                Err(e) => phase.set(Phase::Error(format!("Master build failed: {e}"))),
            }
        });
    };

    let on_skip_join = move |_| { audit_ui_step.set(6); };

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Page header ───────────────────────────────────────────────────
            <div class="s4a-page-hdr">
                <div class="s4a-hdr-left">
                    <span class="s4a-page-step">"Step 4a"</span>
                    <span class="s4a-page-title">"Data relationships"</span>
                    <span class="s4a-page-sub">"Link your datasets before analysis"</span>
                </div>
                <div class="s4a-page-stats">
                    {move || match phase.get() {
                        Phase::Loading | Phase::BuildingMaster => view! {
                            <div style="display:flex;align-items:center;gap:8px;color:var(--w-text-3);font-size:12px">
                                <Spinner size=12 />
                                {move || match phase.get() {
                                    Phase::BuildingMaster => "Building master record…".to_string(),
                                    _ => "Loading…".to_string(),
                                }}
                            </div>
                        }.into_any(),
                        _ => view! { <div></div> }.into_any(),
                    }}
                </div>
            </div>

            // ── Error banner ──────────────────────────────────────────────────
            {move || {
                if let Phase::Error(msg) = phase.get() {
                    Some(view! {
                        <div style="padding:8px 14px;flex-shrink:0">
                            <Banner message=Signal::derive(move || msg.clone()) variant="error" />
                        </div>
                    })
                } else { None }
            }}

            // ── Loading spinner ───────────────────────────────────────────────
            {move || matches!(phase.get(), Phase::Loading).then(|| view! {
                <div style="flex:1;display:flex;align-items:center;justify-content:center;gap:10px;\
                            color:var(--w-text-3);font-size:13px">
                    <Spinner size=16 />
                    "Loading data…"
                </div>
            })}

            // ── Idle: show uploaded tables + detect button ─────────────────────
            {move || (phase.get() == Phase::Idle).then(|| {
                let schemas_snap = schemas.get();

                // ── Detect duplicate table names ──────────────────────────────
                // Group non-master imports by table_name. Any group with >1 entry is a duplicate.
                let mut groups: std::collections::HashMap<String, Vec<SessionSchema>> =
                    std::collections::HashMap::new();
                for s in schemas_snap.iter().filter(|s| s.source_type != "master") {
                    groups.entry(s.table_name.clone()).or_default().push(s.clone());
                }
                let mut dup_groups: Vec<(String, Vec<SessionSchema>)> = groups
                    .into_iter()
                    .filter(|(_, v)| v.len() > 1)
                    .collect();
                dup_groups.sort_by(|a, b| a.0.cmp(&b.0));

                view! {
                    <div style="flex:1;overflow-y:auto;padding:24px;display:flex;flex-direction:column;gap:20px">

                        <div style="font-size:13px;color:var(--w-text-2);max-width:560px;line-height:1.6">
                            "You have uploaded multiple datasets. VinRouge can detect common columns \
                             and link them into a single master record for analysis."
                        </div>

                        // ── Duplicate data section (only shown when duplicates exist) ──
                        {(!dup_groups.is_empty()).then(|| view! {
                            <div>
                                <div style="font-size:10px;text-transform:uppercase;letter-spacing:.06em;\
                                            color:#e8a04a;margin-bottom:10px">
                                    {format!("Duplicate data — {} table{} imported more than once",
                                        dup_groups.len(),
                                        if dup_groups.len() == 1 { "" } else { "s" })}
                                </div>
                                <div style="display:flex;flex-direction:column;gap:10px;max-width:640px">
                                    {dup_groups.into_iter().map(|(name, entries)| {
                                        view! {
                                            <div style="border:0.5px solid #553300;border-radius:4px;overflow:hidden">
                                                <div style="background:#1a1000;padding:7px 14px;font-size:11px;\
                                                            color:#e8a04a;font-family:monospace">
                                                    {name}
                                                </div>
                                                {entries.into_iter().enumerate().map(|(i, entry)| {
                                                    let eid = entry.import_id.clone();
                                                    let rows = entry.row_count;
                                                    let cols = entry.columns.len();
                                                    let is_first = i == 0;
                                                    view! {
                                                        <div style="display:flex;align-items:center;gap:12px;\
                                                                    padding:8px 14px;background:#0d0d0d;\
                                                                    border-top:0.5px solid #222">
                                                            <span style="font-size:10px;color:#555;width:18px;flex-shrink:0">
                                                                {format!("#{}", i + 1)}
                                                            </span>
                                                            <span style="font-size:11px;color:var(--w-text-3);flex:1">
                                                                {format!("{rows} rows · {cols} columns")}
                                                            </span>
                                                            {if is_first {
                                                                view! {
                                                                    <span style="font-size:10px;color:#4ade80;padding:2px 8px;\
                                                                                 border:0.5px solid #2d5a2d;border-radius:3px">
                                                                        "keep"
                                                                    </span>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <button
                                                                        on:click=move |_| {
                                                                            let id = eid.clone();
                                                                            spawn_local(async move {
                                                                                let _ = tauri_invoke_args::<()>(
                                                                                    "delete_session_import",
                                                                                    serde_json::json!({ "importId": id }),
                                                                                ).await;
                                                                                schemas.update(|v| v.retain(|s| s.import_id != id));
                                                                            });
                                                                        }
                                                                        style="padding:2px 10px;background:#2a0d0d;\
                                                                               border:0.5px solid #5a2d2d;color:#e06060;\
                                                                               border-radius:3px;font-size:10px;cursor:pointer"
                                                                    >
                                                                        "Remove"
                                                                    </button>
                                                                }.into_any()
                                                            }}
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        })}

                        // Uploaded tables
                        <div>
                            <div style="font-size:10px;text-transform:uppercase;letter-spacing:.06em;\
                                        color:var(--w-text-3);margin-bottom:10px">
                                "Uploaded tables"
                            </div>
                            <div style="display:flex;flex-direction:column;gap:6px;max-width:600px">
                                {schemas_snap.iter().filter(|s| s.source_type != "master").map(|s| {
                                    let name = s.table_name.clone();
                                    let rows = s.row_count;
                                    let cols = s.columns.len();
                                    view! {
                                        <div style="display:flex;align-items:center;gap:12px;padding:10px 14px;\
                                                    background:#111;border:0.5px solid var(--w-border);\
                                                    border-radius:4px">
                                            <div style="width:8px;height:8px;border-radius:50%;background:#4ade80;flex-shrink:0"></div>
                                            <span style="font-family:monospace;font-size:12px;color:var(--w-text-1);flex:1">{name}</span>
                                            <span style="font-size:11px;color:var(--w-text-3)">{format!("{rows} rows")}</span>
                                            <span style="font-size:11px;color:var(--w-text-3)">{format!("{cols} columns")}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>

                        // Action buttons
                        <div style="display:flex;gap:10px;align-items:center;margin-top:4px">
                            <button
                                style="padding:9px 20px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                       color:#4ade80;border-radius:4px;font-size:13px;cursor:pointer;\
                                       font-weight:500"
                                on:click=on_detect
                            >
                                "Detect Relationships with AI"
                            </button>
                            <button
                                style="padding:9px 16px;background:transparent;border:0.5px solid var(--w-border);\
                                       color:var(--w-text-3);border-radius:4px;font-size:12px;cursor:pointer"
                                on:click=on_skip_join
                            >
                                "Skip — use tables separately"
                            </button>
                        </div>

                        <div style="font-size:11px;color:var(--w-text-3);max-width:500px">
                            "AI detection looks for columns with matching values across tables (e.g. employee IDs). \
                             You can also add links manually after detection."
                        </div>
                    </div>
                }
            })}

            // ── Relationship review panel ──────────────────────────────────────
            {move || (phase.get() == Phase::RelationshipReview).then(|| {
                let candidates = join_candidates.get();
                let schemas_snap = schemas.get();
                let primary_name = schemas_snap.iter()
                    .filter(|s| s.source_type != "master")
                    .max_by_key(|s| s.row_count)
                    .map(|s| s.table_name.clone())
                    .unwrap_or_default();

                view! {
                    <div style="flex:1;overflow-y:auto;padding:20px 24px;display:flex;flex-direction:column;gap:16px">
                        <div style="font-size:13px;color:var(--w-text-2);max-width:600px">
                            "Tick the relationships that are correct, then build a unified master record."
                        </div>

                        <div style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--w-text-3)">
                            <span style="background:#1e3a1e;color:#4ade80;padding:2px 8px;\
                                         border-radius:3px;font-weight:600">
                                "PRIMARY"
                            </span>
                            {primary_name.clone()}
                            " — all rows kept; secondary tables joined in"
                        </div>

                        {candidates.is_empty().then(|| view! {
                            <div style="font-size:12px;color:#f87171;padding:8px 10px;\
                                        background:#1a0a0a;border:0.5px solid #4a1a1a;\
                                        border-radius:4px;max-width:700px">
                                "No relationships were detected automatically. \
                                 Add one manually below, or choose \"Use Separately\"."
                            </div>
                        })}

                        <div style="display:flex;flex-direction:column;gap:6px;max-width:700px">
                            {candidates.into_iter().enumerate().map(|(i, c)| {
                                let is_accepted = accepted_joins.get().get(i).copied().unwrap_or(false);
                                let conf = c.confidence;
                                let bar_color = if conf >= 80 { "#4ade80" } else if conf >= 50 { "#facc15" } else { "#f87171" };
                                let desc = format!("{}.{} ↔ {}.{}", c.left_table, c.left_col, c.right_table, c.right_col);
                                let overlap = c.overlap_count;
                                view! {
                                    <div
                                        style=move || format!(
                                            "display:flex;align-items:center;gap:10px;padding:8px 10px;\
                                             background:{};border-radius:4px;border:0.5px solid {};cursor:pointer",
                                            if is_accepted { "#0f1f0f" } else { "#111" },
                                            if is_accepted { "#2d5a2d" } else { "var(--w-border)" }
                                        )
                                        on:click=move |_| {
                                            accepted_joins.update(|v| {
                                                if let Some(b) = v.get_mut(i) { *b = !*b; }
                                            });
                                        }
                                    >
                                        <div style=move || format!(
                                            "width:14px;height:14px;border-radius:2px;flex-shrink:0;\
                                             background:{};border:1px solid {}",
                                            if is_accepted { "#4ade80" } else { "var(--w-border)" },
                                            if is_accepted { "#4ade80" } else { "var(--w-border)" }
                                        )>
                                            {move || is_accepted.then(|| view! {
                                                <span style="font-size:10px;line-height:14px;\
                                                             display:flex;justify-content:center">"✓"</span>
                                            })}
                                        </div>
                                        <span style="font-family:monospace;font-size:12px;flex:1;color:var(--w-text-1)">{desc}</span>
                                        <span style="font-size:11px;color:var(--w-text-3)">{format!("{overlap} matching values")}</span>
                                        <div style="display:flex;align-items:center;gap:4px;width:80px;flex-shrink:0">
                                            <div style=format!(
                                                "height:4px;background:{};border-radius:2px;width:{}px",
                                                bar_color, (conf as f32 / 100.0 * 64.0) as u8
                                            )></div>
                                            <span style=format!("font-size:10px;color:{bar_color}")>{format!("{conf}%")}</span>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>

                        // Manual add form
                        <div style="border:0.5px solid #333;border-radius:4px;padding:12px;max-width:700px">
                            <div style="font-size:10px;color:var(--w-text-3);margin-bottom:8px;\
                                        text-transform:uppercase;letter-spacing:0.06em">
                                "Add relationship manually"
                            </div>
                            <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
                                <select
                                    style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                           border-radius:3px;padding:4px 6px;font-size:11px"
                                    on:change=move |ev| {
                                        manual_left_table.set(event_target_value(&ev));
                                        manual_left_col.set(String::new());
                                    }
                                >
                                    <option value="">"— left table —"</option>
                                    {schemas_snap.iter().filter(|s| s.source_type != "master").map(|s| {
                                        let tn = s.table_name.clone();
                                        let tn2 = tn.clone();
                                        view! { <option value={tn}>{tn2}</option> }
                                    }).collect_view()}
                                </select>
                                {move || {
                                    let lt = manual_left_table.get();
                                    let cols = schemas.get().iter()
                                        .find(|s| s.table_name == lt)
                                        .map(|s| s.columns.clone())
                                        .unwrap_or_default();
                                    view! {
                                        <select
                                            style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                                   border-radius:3px;padding:4px 6px;font-size:11px"
                                            on:change=move |ev| manual_left_col.set(event_target_value(&ev))
                                        >
                                            <option value="">"— column —"</option>
                                            {cols.into_iter().map(|c| {
                                                let c2 = c.clone();
                                                view! { <option value={c}>{c2}</option> }
                                            }).collect_view()}
                                        </select>
                                    }
                                }}
                                <span style="color:#555;font-size:13px">"↔"</span>
                                <select
                                    style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                           border-radius:3px;padding:4px 6px;font-size:11px"
                                    on:change=move |ev| {
                                        manual_right_table.set(event_target_value(&ev));
                                        manual_right_col.set(String::new());
                                    }
                                >
                                    <option value="">"— right table —"</option>
                                    {schemas_snap.iter().filter(|s| s.source_type != "master").map(|s| {
                                        let tn = s.table_name.clone();
                                        let tn2 = tn.clone();
                                        view! { <option value={tn}>{tn2}</option> }
                                    }).collect_view()}
                                </select>
                                {move || {
                                    let rt = manual_right_table.get();
                                    let cols = schemas.get().iter()
                                        .find(|s| s.table_name == rt)
                                        .map(|s| s.columns.clone())
                                        .unwrap_or_default();
                                    view! {
                                        <select
                                            style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                                   border-radius:3px;padding:4px 6px;font-size:11px"
                                            on:change=move |ev| manual_right_col.set(event_target_value(&ev))
                                        >
                                            <option value="">"— column —"</option>
                                            {cols.into_iter().map(|c| {
                                                let c2 = c.clone();
                                                view! { <option value={c}>{c2}</option> }
                                            }).collect_view()}
                                        </select>
                                    }
                                }}
                                <button
                                    style="padding:4px 12px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                           color:#4ade80;border-radius:3px;font-size:11px;cursor:pointer"
                                    on:click=on_add_manual_join
                                >
                                    "+ Add"
                                </button>
                            </div>
                        </div>

                        <div style="display:flex;gap:10px;margin-top:8px">
                            <button
                                style="padding:7px 18px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                       color:#4ade80;border-radius:4px;font-size:12px;cursor:pointer"
                                on:click=on_build_master
                            >
                                "Build Master Record →"
                            </button>
                            <button
                                style="padding:7px 14px;background:transparent;border:0.5px solid var(--w-border);\
                                       color:var(--w-text-3);border-radius:4px;font-size:12px;cursor:pointer"
                                on:click=on_skip_join
                            >
                                "Use Separately →"
                            </button>
                        </div>
                    </div>
                }
            })}

            // ── Building master spinner ───────────────────────────────────────
            {move || (phase.get() == Phase::BuildingMaster).then(|| view! {
                <div style="flex:1;display:flex;align-items:center;justify-content:center;gap:10px;\
                            color:var(--w-text-3);font-size:13px">
                    <Spinner size=16 />
                    "Building master record…"
                </div>
            })}

            // ── Status bar ────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class="s4-dot s4-dot--idle"></span>
                <span class="s4-status-text">
                    {move || match phase.get() {
                        Phase::Loading        => "Loading imported data…".to_string(),
                        Phase::Idle           => format!("{} tables ready", schemas.get().len()),
                        Phase::BuildingMaster => "Building master record…".to_string(),
                        Phase::RelationshipReview => "Confirm how your datasets connect".to_string(),
                        Phase::Error(e)       => format!("Error: {e}"),
                        _                     => String::new(),
                    }}
                </span>
                <div style="margin-left:auto">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(4)) />
                </div>
            </div>
        </div>
    }
}
