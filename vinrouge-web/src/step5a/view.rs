use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{GhostButton, StatCard};
use crate::ipc::tauri_invoke;
use crate::types::{DslScript, TestResult};

use super::modal::FindingModal;

// ── Step5aView — Audit findings ───────────────────────────────────────────────

#[component]
pub fn Step5aView(
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let scripts: RwSignal<Vec<DslScript>>  = RwSignal::new(vec![]);
    let results: RwSignal<Vec<TestResult>> = RwSignal::new(vec![]);
    let loading: RwSignal<bool>            = RwSignal::new(true);

    // (script_id, script, result) of the finding whose modal is open.
    let modal: RwSignal<Option<(DslScript, TestResult)>> = RwSignal::new(None);

    spawn_local(async move {
        let s: Vec<DslScript>  = tauri_invoke("list_dsl_scripts").await.unwrap_or_default();
        let r: Vec<TestResult> = tauri_invoke("list_test_results").await.unwrap_or_default();
        scripts.set(s);
        results.set(r);
        loading.set(false);
    });

    let finding_count = move || {
        results.get().iter()
            .filter(|r| r.failed_count > 0 || r.error_count > 0)
            .count()
    };
    let pass_count = move || {
        results.get().iter()
            .filter(|r| r.failed_count == 0 && r.error_count == 0)
            .count()
    };

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Finding modal (portal overlay) ────────────────────────────────
            {move || modal.get().map(|(script, result)| {
                view! {
                    <FindingModal
                        script=script
                        result=result
                        on_close=Callback::new(move |()| modal.set(None))
                    />
                }
            })}

            // ── Header ────────────────────────────────────────────────────────
            <div class="s4-header">
                <div style="display:flex;align-items:center;gap:10px;margin-bottom:3px">
                    <span class="s4-badge">"Step 5a"</span>
                    <span class="s4-title">"Audit findings"</span>
                </div>
                <div class="s4-subtitle">
                    "Controls with failed assertions or errors — click a finding to view evidence and charts"
                </div>
            </div>

            // ── Content ───────────────────────────────────────────────────────
            <div style="flex:1;overflow-y:auto;padding:14px">

                {move || loading.get().then(|| view! {
                    <div class="s4-uploading">
                        <crate::components::Spinner size=14 />
                        " Loading findings…"
                    </div>
                })}

                {move || {
                    let s = scripts.get();
                    let r = results.get();

                    if s.is_empty() && !loading.get() {
                        return Some(view! {
                            <div style="color:var(--w-text-3);font-size:13px;padding:20px 0">
                                "No test results found. Complete Step 4b first."
                            </div>
                        }.into_any());
                    }

                    let failing: Vec<_> = s.iter()
                        .filter_map(|script| {
                            let res = r.iter().find(|tr| tr.script_id == script.id)?;
                            if res.failed_count > 0 || res.error_count > 0 {
                                Some((script.clone(), res.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();

                    if failing.is_empty() && !loading.get() {
                        return Some(view! {
                            <div style="color:var(--w-text-3);font-size:13px;padding:20px 0">
                                "All controls passed — no findings to report."
                            </div>
                        }.into_any());
                    }

                    let items = failing.into_iter().map(|(script, res)| {
                        let risk_color = if res.error_count > 0 && res.failed_count == 0 {
                            "var(--w-text-3)"
                        } else {
                            "#e05c5c"
                        };
                        let issue_count = res.failed_count + res.error_count;
                        let script_clone = script.clone();
                        let res_clone    = res.clone();

                        view! {
                            <div class="s5-script-card"
                                style="margin-bottom:10px;cursor:pointer"
                                on:click=move |_| modal.set(Some((script_clone.clone(), res_clone.clone())))>
                                <div class="s5-script-header s5-header-fail"
                                    style="display:flex;align-items:center;
                                           gap:8px;padding:10px 12px">
                                    <div style="flex:1">
                                        <div style=format!(
                                            "font-size:13px;font-weight:600;color:{risk_color}"
                                        )>
                                            {format!("FINDING — {}", script.label)}
                                        </div>
                                        <div style="font-size:11px;color:var(--w-text-3)">
                                            {format!(
                                                "Control {} · {} failed · {} errors · {} passed",
                                                script.control_ref,
                                                res.failed_count,
                                                res.error_count,
                                                res.passed_count
                                            )}
                                        </div>
                                    </div>
                                    <span class="s4-file-badge-mapping">
                                        {format!("{issue_count} issue(s)")}
                                    </span>
                                    <span style="font-size:11px;color:var(--w-text-3)">"→"</span>
                                </div>
                            </div>
                        }
                    }).collect_view();

                    Some(view! { <div>{items}</div> }.into_any())
                }}
            </div>

            // ── Stats bar ─────────────────────────────────────────────────────
            {move || (!results.get().is_empty()).then(move || view! {
                <div style="flex-shrink:0;display:flex;gap:8px;padding:8px 14px;\
                            border-top:0.5px solid var(--w-border)">
                    <StatCard label="Findings"
                        value=Signal::derive(move || finding_count().to_string()) />
                    <StatCard label="Passed"
                        value=Signal::derive(move || pass_count().to_string())
                        green=true />
                </div>
            })}

            // ── Status bar ────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || {
                    if finding_count() > 0 { "s4-dot s4-dot--idle" } else { "s4-dot s4-dot--ready" }
                }></span>
                <span class="s4-status-text">
                    {move || {
                        if loading.get() { return "Loading…".to_string(); }
                        let f = finding_count();
                        let p = pass_count();
                        if f == 0 && p == 0 { return "No results available".to_string(); }
                        format!("{f} finding(s) · {p} control(s) passed")
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(7)) />
                    <GhostButton label="Generate Report →"
                        disabled=Signal::derive(move || results.get().is_empty())
                        on_click=Callback::new(move |()| audit_ui_step.set(9)) />
                </div>
            </div>

        </div>
    }
}
