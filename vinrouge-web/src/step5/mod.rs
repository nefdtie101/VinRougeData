use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{GhostButton, StatCard};
use crate::ipc::tauri_invoke;
use crate::types::{DslScript, TestResult};

// ── Step5View — Audit test results ────────────────────────────────────────────

#[component]
pub fn Step5View(
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    let scripts: RwSignal<Vec<DslScript>> = RwSignal::new(vec![]);
    let results: RwSignal<Vec<TestResult>> = RwSignal::new(vec![]);
    let loading: RwSignal<bool> = RwSignal::new(true);
    let expanded: RwSignal<Option<String>> = RwSignal::new(None); // script_id

    spawn_local(async move {
        let s: Vec<DslScript> = tauri_invoke("list_dsl_scripts").await.unwrap_or_default();
        let r: Vec<TestResult> = tauri_invoke("list_test_results").await.unwrap_or_default();
        scripts.set(s);
        results.set(r);
        loading.set(false);
    });

    // ── Derived stats ─────────────────────────────────────────────────────────
    let total_asserts = move || {
        results
            .get()
            .iter()
            .map(|r| r.passed_count + r.failed_count)
            .sum::<i64>()
    };
    let total_passed = move || {
        results.get().iter().map(|r| r.passed_count).sum::<i64>()
    };
    let total_failed = move || {
        results.get().iter().map(|r| r.failed_count).sum::<i64>()
    };
    let total_errors = move || {
        results.get().iter().map(|r| r.error_count).sum::<i64>()
    };

    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Header ────────────────────────────────────────────────────────
            <div class="s4-header">
                <div style="display:flex;align-items:center;gap:10px;margin-bottom:3px">
                    <span class="s4-badge">"Step 5"</span>
                    <span class="s4-title">"Audit test results"</span>
                </div>
                <div class="s4-subtitle">
                    "DSL assertion and sampling results from the executed audit test scripts"
                </div>
            </div>

            // ── Content ───────────────────────────────────────────────────────
            <div style="flex:1;overflow-y:auto;padding:14px">

                {move || loading.get().then(|| view! {
                    <div class="s4-uploading">
                        <crate::components::Spinner size=14 />
                        " Loading results…"
                    </div>
                })}

                {move || {
                    let s = scripts.get();
                    let r = results.get();
                    if s.is_empty() && !loading.get() {
                        return Some(view! {
                            <div style="color:var(--w-text-3);font-size:13px;padding:20px 0">
                                "No test results found. Complete Step 4a first."
                            </div>
                        }.into_any());
                    }
                    let items = s.into_iter().map(|script| {
                        let sid = script.id.clone();
                        let sid2 = sid.clone();
                        let result = r.iter().find(|tr| tr.script_id == sid).cloned();

                        let (header_cls, badge_cls, badge_txt) = match &result {
                            None => ("s5-script-header", "s4-file-badge-pending", "No result"),
                            Some(tr) if tr.error_count > 0 && tr.passed_count == 0 && tr.failed_count == 0 => {
                                ("s5-script-header s5-header-error", "s4-file-badge-pending", "Error")
                            }
                            Some(tr) if tr.failed_count > 0 => {
                                ("s5-script-header s5-header-fail", "s4-file-badge-mapping", "Fail")
                            }
                            Some(_) => {
                                ("s5-script-header s5-header-pass", "s4-file-badge-mapped", "Pass")
                            }
                        };

                        let summary = result.as_ref().map(|tr| format!(
                            "✓ {} passed · ✗ {} failed · ⚠ {} errors",
                            tr.passed_count, tr.failed_count, tr.error_count
                        ));

                        // Store raw result data to be rendered lazily inside the expand closure
                        let result_data: Vec<serde_json::Value> =
                            result.as_ref().map(|tr| tr.results.clone()).unwrap_or_default();
                        let script_text = script.script_text.clone();

                        let sid3 = sid2.clone();
                        let sid4 = sid2.clone();
                        let sid5 = sid2.clone();
                        let sid6 = sid2.clone();

                        let toggle = move |_| {
                            if expanded.get_untracked().as_deref() == Some(sid4.as_str()) {
                                expanded.set(None);
                            } else {
                                expanded.set(Some(sid4.clone()));
                            }
                        };

                        view! {
                            <div class="s5-script-card" style="margin-bottom:8px">
                                <div class=header_cls
                                    style="cursor:pointer;display:flex;align-items:center;gap:8px;padding:10px 12px"
                                    on:click=toggle>
                                    <span style="font-size:11px;color:var(--w-text-3);min-width:14px">
                                        {move || if expanded.get().as_deref() == Some(sid3.as_str()) { "▾" } else { "▸" }}
                                    </span>
                                    <div style="flex:1">
                                        <div style="font-size:13px;font-weight:600;color:var(--w-text-1)">
                                            {script.label.clone()}
                                        </div>
                                        <div style="font-size:11px;color:var(--w-text-3)">
                                            {format!("Control {}", script.control_ref)}
                                            {summary.map(|s| format!(" — {s}"))}
                                        </div>
                                    </div>
                                    <span class=badge_cls>{badge_txt}</span>
                                </div>
                                {move || (expanded.get().as_deref() == Some(sid5.as_str())).then(|| {
                                    let stmt_views = result_data.iter().map(|stmt| {
                                        match stmt["kind"].as_str().unwrap_or("") {
                                            "assert" => {
                                                let passed = stmt["passed"].as_bool().unwrap_or(false);
                                                let label = stmt["label"].as_str()
                                                    .map(|s| s.to_string())
                                                    .unwrap_or_else(|| format!("Assert #{}", stmt["index"].as_u64().unwrap_or(0)));
                                                let lhs = stmt["lhs_value"].as_str().unwrap_or("?").to_string();
                                                let op = stmt["op"].as_str().unwrap_or("=").to_string();
                                                let rhs = stmt["rhs_value"].as_str().unwrap_or("?").to_string();
                                                let (row_cls, icon) = if passed {
                                                    ("s5-stmt-row s5-stmt-pass", "✓")
                                                } else {
                                                    ("s5-stmt-row s5-stmt-fail", "✗")
                                                };
                                                view! {
                                                    <div class=row_cls>
                                                        <span class="s5-stmt-icon">{icon}</span>
                                                        <span class="s5-stmt-label">{label}</span>
                                                        <span class="s5-stmt-values">{format!("{lhs} {op} {rhs}")}</span>
                                                    </div>
                                                }.into_any()
                                            }
                                            "sample" => {
                                                let method = stmt["method"].as_str().unwrap_or("Sample").to_string();
                                                let pop = stmt["population_size"].as_u64().unwrap_or(0);
                                                let sel = stmt["selected_count"].as_u64().unwrap_or(0);
                                                view! {
                                                    <div class="s5-stmt-row s5-stmt-sample">
                                                        <span class="s5-stmt-icon">"◎"</span>
                                                        <span class="s5-stmt-label">{method}</span>
                                                        <span class="s5-stmt-values">{format!("{sel} selected from {pop} items")}</span>
                                                    </div>
                                                }.into_any()
                                            }
                                            "value" => {
                                                let val = stmt["value"].as_str().unwrap_or("").to_string();
                                                view! {
                                                    <div class="s5-stmt-row">
                                                        <span class="s5-stmt-icon">"="</span>
                                                        <span class="s5-stmt-label">"Value"</span>
                                                        <span class="s5-stmt-values">{val}</span>
                                                    </div>
                                                }.into_any()
                                            }
                                            _ => {
                                                let err = stmt["error"].as_str().unwrap_or("Unknown error").to_string();
                                                view! {
                                                    <div class="s5-stmt-row s5-stmt-error">
                                                        <span class="s5-stmt-icon">"⚠"</span>
                                                        <span class="s5-stmt-label">"Error"</span>
                                                        <span class="s5-stmt-values">{err}</span>
                                                    </div>
                                                }.into_any()
                                            }
                                        }
                                    }).collect_view();
                                    view! {
                                        <div style="border-top:0.5px solid var(--w-border);padding:8px 12px">
                                            <div style="font-family:monospace;font-size:11px;\
                                                        color:var(--w-text-3);padding:8px;\
                                                        background:var(--w-bg-2);border-radius:4px;\
                                                        margin-bottom:8px;white-space:pre-wrap">
                                                {script_text.clone()}
                                            </div>
                                            {stmt_views}
                                        </div>
                                    }
                                })}
                            </div>
                        }
                    }).collect_view();

                    Some(view! { <div>{items}</div> }.into_any())
                }}
            </div>

            
            {move || {
                (!results.get().is_empty()).then(move || view! {
                    <div style="flex-shrink:0;display:flex;gap:8px;padding:8px 14px;\
                                border-top:0.5px solid var(--w-border)">
                        <StatCard label="Assertions"
                            value=Signal::derive(move || total_asserts().to_string()) />
                        <StatCard label="Passed"
                            value=Signal::derive(move || total_passed().to_string())
                            green=true />
                        <StatCard label="Failed"
                            value=Signal::derive(move || total_failed().to_string()) />
                        <StatCard label="Errors"
                            value=Signal::derive(move || total_errors().to_string()) />
                    </div>
                })
            }}

            // ── Status bar ────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || {
                    if total_failed() > 0 || total_errors() > 0 {
                        "s4-dot s4-dot--idle"
                    } else if total_passed() > 0 {
                        "s4-dot s4-dot--ready"
                    } else {
                        "s4-dot s4-dot--idle"
                    }
                }></span>
                <span class="s4-status-text">
                    {move || {
                        if loading.get() { return "Loading…".to_string(); }
                        let s = scripts.get().len();
                        if s == 0 { return "No results available".to_string(); }
                        format!(
                            "{s} test scripts · {} passed · {} failed",
                            total_passed(), total_failed()
                        )
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(6)) />
                </div>
            </div>

        </div>
    }
}
