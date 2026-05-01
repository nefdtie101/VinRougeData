use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, Spinner};
use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::types::{AuditProcessWithControls, SessionSchema};
use super::types::Phase;

#[component]
pub fn Step4aView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let _ = audit_plan;
    let _ = status;

    let phase: RwSignal<Phase>               = RwSignal::new(Phase::Loading);
    let schemas: RwSignal<Vec<SessionSchema>> = RwSignal::new(vec![]);

    // Poll until imports are ready, then auto-advance to Algorithm Review.
    spawn_local(async move {
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
                phase.set(Phase::Loading);
                gloo_timers::future::TimeoutFuture::new(1_000).await;
                continue;
            }

            schemas.set(session_schemas);
            phase.set(Phase::Idle);
            break;
        }
    });

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Page header ───────────────────────────────────────────────────
            <div class="s4a-page-hdr">
                <div class="s4a-hdr-left">
                    <span class="s4a-page-step">"Step 4a"</span>
                    <span class="s4a-page-title">"Uploaded tables"</span>
                    <span class="s4a-page-sub">"Review your data before algorithm generation"</span>
                </div>
                <div class="s4a-page-stats">
                    {move || matches!(phase.get(), Phase::Loading).then(|| view! {
                        <div style="display:flex;align-items:center;gap:8px;color:var(--w-text-3);font-size:12px">
                            <Spinner size=12 />
                            "Loading…"
                        </div>
                    })}
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

            // ── Loading ───────────────────────────────────────────────────────
            {move || matches!(phase.get(), Phase::Loading).then(|| view! {
                <div style="flex:1;display:flex;align-items:center;justify-content:center;gap:10px;\
                            color:var(--w-text-3);font-size:13px">
                    <Spinner size=16 />
                    "Waiting for data import to finish…"
                </div>
            })}

            // ── Idle: show uploaded tables ─────────────────────────────────────
            {move || (phase.get() == Phase::Idle).then(|| {
                let schemas_snap = schemas.get();

                // Warn about duplicate table names (same file uploaded twice)
                let mut groups: std::collections::HashMap<String, Vec<SessionSchema>> =
                    std::collections::HashMap::new();
                for s in &schemas_snap {
                    groups.entry(s.table_name.clone()).or_default().push(s.clone());
                }
                let mut dup_groups: Vec<(String, Vec<SessionSchema>)> = groups
                    .into_iter()
                    .filter(|(_, v)| v.len() > 1)
                    .collect();
                dup_groups.sort_by(|a, b| a.0.cmp(&b.0));

                view! {
                    <div style="flex:1;overflow-y:auto;padding:24px;display:flex;flex-direction:column;gap:20px">

                        // ── Duplicate warning ─────────────────────────────────
                        {(!dup_groups.is_empty()).then(|| view! {
                            <div>
                                <div style="font-size:10px;text-transform:uppercase;letter-spacing:.06em;\
                                            color:#e8a04a;margin-bottom:10px">
                                    {format!("Duplicate data — {} table{} imported more than once",
                                        dup_groups.len(),
                                        if dup_groups.len() == 1 { "" } else { "s" })}
                                </div>
                                <div style="display:flex;flex-direction:column;gap:10px;max-width:640px">
                                    {dup_groups.into_iter().map(|(name, entries)| view! {
                                        <div style="border:0.5px solid #553300;border-radius:4px;overflow:hidden">
                                            <div style="background:#1a1000;padding:7px 14px;font-size:11px;\
                                                        color:#e8a04a;font-family:monospace">
                                                {name}
                                            </div>
                                            {entries.into_iter().enumerate().map(|(i, entry)| {
                                                let eid  = entry.import_id.clone();
                                                let rows = entry.row_count;
                                                let cols = entry.columns.len();
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
                                                        {if i == 0 {
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
                                                                        schemas.update(|v| v.retain(|s| s.import_id != id));
                                                                        spawn_local(async move {
                                                                            let _ = tauri_invoke_args::<()>(
                                                                                "delete_session_import",
                                                                                serde_json::json!({ "importId": id }),
                                                                            ).await;
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
                                    }).collect_view()}
                                </div>
                            </div>
                        })}

                        // ── Uploaded tables ───────────────────────────────────
                        <div>
                            <div style="font-size:10px;text-transform:uppercase;letter-spacing:.06em;\
                                        color:var(--w-text-3);margin-bottom:10px">
                                "Uploaded tables"
                            </div>
                            <div style="display:flex;flex-direction:column;gap:6px;max-width:600px">
                                {schemas_snap.iter().map(|s| {
                                    let id   = s.import_id.clone();
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
                                            <button
                                                on:click=move |_| {
                                                    let import_id = id.clone();
                                                    schemas.update(|v| v.retain(|s| s.import_id != import_id));
                                                    spawn_local(async move {
                                                        let _ = tauri_invoke_args::<()>(
                                                            "delete_session_import",
                                                            serde_json::json!({ "importId": import_id }),
                                                        ).await;
                                                    });
                                                }
                                                style="padding:2px 8px;background:transparent;\
                                                       border:0.5px solid #333;color:#666;\
                                                       border-radius:3px;font-size:11px;cursor:pointer;\
                                                       line-height:1;flex-shrink:0"
                                                title="Remove this import"
                                            >
                                                "×"
                                            </button>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>

                        // ── Proceed button ────────────────────────────────────
                        <div style="margin-top:4px">
                            <button
                                style="padding:9px 20px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                       color:#4ade80;border-radius:4px;font-size:13px;cursor:pointer;\
                                       font-weight:500"
                                on:click=move |_| audit_ui_step.set(6)
                            >
                                "Proceed to Algorithm Review →"
                            </button>
                        </div>
                    </div>
                }
            })}

            // ── Status bar ────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class="s4-dot s4-dot--idle"></span>
                <span class="s4-status-text">
                    {move || match phase.get() {
                        Phase::Loading    => "Waiting for import…".to_string(),
                        Phase::Idle       => format!("{} table(s) ready", schemas.get().len()),
                        Phase::Error(e)   => format!("Error: {e}"),
                        _                 => String::new(),
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
