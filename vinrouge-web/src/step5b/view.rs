use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{GhostButton, PrimaryButton, Spinner};
use crate::ipc::tauri_invoke;
use crate::types::{AuditProcessWithControls, DslScript, TestResult};
use super::pipeline::do_generate_report;

// ── Step5bView — Audit report generation ─────────────────────────────────────

#[component]
pub fn Step5bView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let scripts: RwSignal<Vec<DslScript>>           = RwSignal::new(vec![]);
    let results: RwSignal<Vec<TestResult>>           = RwSignal::new(vec![]);
    let loading: RwSignal<bool>                      = RwSignal::new(true);
    let generating: RwSignal<bool>                   = RwSignal::new(false);
    let report: RwSignal<Option<serde_json::Value>>  = RwSignal::new(None);
    let expanded_finding: RwSignal<Option<String>>   = RwSignal::new(None);

    spawn_local(async move {
        let s: Vec<DslScript>  = tauri_invoke("list_dsl_scripts").await.unwrap_or_default();
        let r: Vec<TestResult> = tauri_invoke("list_test_results").await.unwrap_or_default();
        scripts.set(s);
        results.set(r);
        loading.set(false);
    });

    let on_generate = move |()| {
        do_generate_report(
            scripts.get_untracked(),
            results.get_untracked(),
            audit_plan.get_untracked(),
            report,
            generating,
            status,
        );
    };

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Header ────────────────────────────────────────────────────────
            <div class="s4-header">
                <div style="display:flex;align-items:center;gap:10px;margin-bottom:3px">
                    <span class="s4-badge">"Step 5b"</span>
                    <span class="s4-title">"Audit report"</span>
                </div>
                <div class="s4-subtitle">
                    "AI-generated internal audit report based on test execution findings"
                </div>
            </div>

            // ── Content ───────────────────────────────────────────────────────
            <div style="flex:1;overflow-y:auto;padding:14px">

                {move || loading.get().then(|| view! {
                    <div class="s4-uploading">
                        <Spinner size=14 />
                        " Loading data…"
                    </div>
                })}

                // ── No results guard ──────────────────────────────────────────
                {move || {
                    (!loading.get() && results.get().is_empty()).then(|| view! {
                        <div style="color:var(--w-text-3);font-size:13px;padding:20px 0">
                            "No test results available. Complete Step 4b first."
                        </div>
                    })
                }}

                // ── Generate prompt ───────────────────────────────────────────
                {move || {
                    (!loading.get() && !results.get().is_empty() && report.get().is_none()
                        && !generating.get()).then(|| view! {
                        <div style="display:flex;flex-direction:column;align-items:center;\
                                    justify-content:center;padding:48px 0;gap:16px">
                            <div style="font-size:14px;color:var(--w-text-2);text-align:center;\
                                        max-width:400px;line-height:1.6">
                                "Generate a formal internal audit report from the test results. \
                                 The AI will write structured findings with evidence and recommendations."
                            </div>
                            <PrimaryButton
                                label="Generate Audit Report"
                                on_click=Callback::new(on_generate)
                            />
                        </div>
                    })
                }}

                // ── Generating spinner ────────────────────────────────────────
                {move || generating.get().then(|| view! {
                    <div class="s4-uploading">
                        <Spinner size=14 />
                        " Generating audit report…"
                    </div>
                })}

                // ── Report display ────────────────────────────────────────────
                {move || report.get().map(|v| {
                    let exec_summary = v["executive_summary"].as_str().unwrap_or("").to_string();
                    let overall_risk = v["overall_risk"].as_str().unwrap_or("").to_string();
                    let conclusion   = v["overall_conclusion"].as_str().unwrap_or("").to_string();

                    let findings: Vec<serde_json::Value> =
                        v["findings"].as_array().cloned().unwrap_or_default();
                    let passed: Vec<String> = v["passed_controls"]
                        .as_array().unwrap_or(&vec![])
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect();

                    let risk_color = match overall_risk.as_str() {
                        "High"   => "#e05c5c",
                        "Medium" => "#f0a500",
                        _        => "var(--w-accent)",
                    };

                    let finding_items = findings.into_iter().map(|f| {
                        let ctrl_ref  = f["control_ref"].as_str().unwrap_or("").to_string();
                        let ctrl_name = f["control_name"].as_str().unwrap_or("").to_string();
                        let risk      = f["risk_level"].as_str().unwrap_or("Medium").to_string();
                        let finding   = f["finding"].as_str().unwrap_or("").to_string();
                        let evidence  = f["evidence"].as_str().unwrap_or("").to_string();
                        let rec       = f["recommendation"].as_str().unwrap_or("").to_string();

                        let key  = ctrl_ref.clone();
                        let key2 = key.clone();
                        let key3 = key.clone();

                        let rc = match risk.as_str() {
                            "High"   => "#e05c5c",
                            "Medium" => "#f0a500",
                            _        => "var(--w-accent)",
                        };

                        let toggle = move |_| {
                            if expanded_finding.get_untracked().as_deref() == Some(key2.as_str()) {
                                expanded_finding.set(None);
                            } else {
                                expanded_finding.set(Some(key2.clone()));
                            }
                        };

                        view! {
                            <div class="s5-script-card" style="margin-bottom:8px">
                                <div style=format!(
                                        "cursor:pointer;display:flex;align-items:center;\
                                         gap:8px;padding:10px 12px;\
                                         border-left:3px solid {rc}"
                                    )
                                    on:click=toggle>
                                    <span style="font-size:11px;color:var(--w-text-3);min-width:14px">
                                        {move || if expanded_finding.get().as_deref() == Some(key.as_str()) { "▾" } else { "▸" }}
                                    </span>
                                    <div style="flex:1">
                                        <div style="font-size:13px;font-weight:600;color:var(--w-text-1)">
                                            {ctrl_name}
                                        </div>
                                        <div style="font-size:11px;color:var(--w-text-3)">{ctrl_ref}</div>
                                    </div>
                                    <span style=format!(
                                        "font-size:10px;font-weight:600;color:{rc};\
                                         background:rgba(0,0,0,0.06);padding:2px 8px;\
                                         border-radius:3px"
                                    )>{risk}</span>
                                </div>
                                {move || (expanded_finding.get().as_deref() == Some(key3.as_str())).then(|| view! {
                                    <div style="border-top:0.5px solid var(--w-border);\
                                                padding:12px 16px;display:flex;flex-direction:column;gap:10px">
                                        <div>
                                            <div style="font-size:11px;font-weight:600;\
                                                        color:var(--w-text-3);margin-bottom:3px">"FINDING"</div>
                                            <div style="font-size:12px;color:var(--w-text-1);line-height:1.5">
                                                {finding.clone()}
                                            </div>
                                        </div>
                                        <div>
                                            <div style="font-size:11px;font-weight:600;\
                                                        color:var(--w-text-3);margin-bottom:3px">"EVIDENCE"</div>
                                            <div style="font-size:12px;color:var(--w-text-2);\
                                                        font-family:monospace;line-height:1.5">
                                                {evidence.clone()}
                                            </div>
                                        </div>
                                        <div>
                                            <div style="font-size:11px;font-weight:600;\
                                                        color:var(--w-accent);margin-bottom:3px">"RECOMMENDATION"</div>
                                            <div style="font-size:12px;color:var(--w-text-1);line-height:1.5">
                                                {rec.clone()}
                                            </div>
                                        </div>
                                    </div>
                                })}
                            </div>
                        }
                    }).collect_view();

                    view! {
                        <div>
                            // Executive summary
                            <div style="background:var(--w-bg-2);border-radius:6px;\
                                        padding:14px 16px;margin-bottom:16px">
                                <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px">
                                    <span style="font-size:12px;font-weight:700;color:var(--w-text-1)">
                                        "EXECUTIVE SUMMARY"
                                    </span>
                                    <span style=format!(
                                        "font-size:10px;font-weight:600;color:{risk_color};\
                                         background:rgba(0,0,0,0.06);padding:2px 8px;border-radius:3px"
                                    )>
                                        {format!("Overall risk: {overall_risk}")}
                                    </span>
                                </div>
                                <div style="font-size:12px;color:var(--w-text-1);line-height:1.6">
                                    {exec_summary}
                                </div>
                            </div>

                            // Findings
                            <div style="margin-bottom:16px">
                                <div style="font-size:11px;font-weight:600;color:var(--w-text-3);\
                                            margin-bottom:8px;text-transform:uppercase;letter-spacing:0.05em">
                                    "Findings"
                                </div>
                                {finding_items}
                            </div>

                            // Passed controls
                            {(!passed.is_empty()).then(|| view! {
                                <div style="background:rgba(56,167,73,0.06);border-radius:6px;\
                                            padding:10px 14px;margin-bottom:16px;\
                                            border-left:3px solid #38a749">
                                    <div style="font-size:11px;font-weight:600;color:#38a749;margin-bottom:4px">
                                        "CONTROLS PASSED"
                                    </div>
                                    <div style="font-size:12px;color:var(--w-text-2)">
                                        {passed.join("  ·  ")}
                                    </div>
                                </div>
                            })}

                            // Conclusion
                            <div style="background:var(--w-bg-2);border-radius:6px;padding:14px 16px">
                                <div style="font-size:11px;font-weight:600;color:var(--w-text-3);\
                                            margin-bottom:6px;text-transform:uppercase;letter-spacing:0.05em">
                                    "Overall conclusion"
                                </div>
                                <div style="font-size:12px;color:var(--w-text-1);line-height:1.6">
                                    {conclusion}
                                </div>
                            </div>

                            // Regenerate
                            <div style="margin-top:16px;display:flex;justify-content:flex-end">
                                <PrimaryButton
                                    label="Regenerate Report"
                                    loading=Signal::derive(move || generating.get())
                                    loading_label=Some("Generating…")
                                    on_click=Callback::new(on_generate)
                                />
                            </div>
                        </div>
                    }.into_any()
                })}

            </div>

            // ── Status bar ────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || {
                    if report.get().is_some() { "s4-dot s4-dot--ready" } else { "s4-dot s4-dot--idle" }
                }></span>
                <span class="s4-status-text">
                    {move || {
                        if loading.get()          { return "Loading…".to_string(); }
                        if generating.get()       { return "Generating audit report…".to_string(); }
                        if report.get().is_some() { return "Report generated".to_string(); }
                        "Ready to generate".to_string()
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(8)) />
                </div>
            </div>

        </div>
    }
}
