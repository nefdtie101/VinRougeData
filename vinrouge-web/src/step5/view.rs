use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;

use crate::components::{GhostButton, StatCard};
use crate::ipc::tauri_invoke;
use crate::types::{DslScript, TestResult};
use crate::step5a::chart::{EChart, RawChart, build_summary_option, pick_sample_column, sample_distribution};
use crate::step5a::types::{ChartTab, DistPoint};

fn export_debug(scripts: &[DslScript], results: &[TestResult]) {
    let rows: Vec<serde_json::Value> = scripts.iter().map(|s| {
        let res = results.iter().find(|r| r.script_id == s.id);
        serde_json::json!({
            "control_ref":  s.control_ref,
            "label":        s.label,
            "script_id":    s.id,
            "script_text":  s.script_text,
            "result": res.map(|r| serde_json::json!({
                "passed_count": r.passed_count,
                "failed_count": r.failed_count,
                "error_count":  r.error_count,
                "statements":   r.results,
            }))
        })
    }).collect();

    let payload = serde_json::json!({
        "exported_at": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "scripts": rows,
    });

    let json = serde_json::to_string_pretty(&payload).unwrap_or_default();
    let window = web_sys::window().unwrap();
    if let Ok(f) = js_sys::Reflect::get(&window, &JsValue::from_str("vinrougeDownload")) {
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            let _ = f.call2(
                &JsValue::UNDEFINED,
                &JsValue::from_str("vinrouge_debug.json"),
                &JsValue::from_str(&json),
            );
        }
    }
}

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

                // ── Overview chart — pass/fail/error per control ───────────────
                {move || {
                    let s = scripts.get();
                    let r = results.get();
                    if s.is_empty() || r.is_empty() { return None; }

                    let opt = {
                        let items: Vec<(&str, i64, i64, i64)> = s.iter().filter_map(|sc| {
                            let res = r.iter().find(|tr| tr.script_id == sc.id)?;
                            Some((sc.control_ref.as_str(), res.passed_count, res.failed_count, res.error_count))
                        }).collect();
                        build_summary_option(&items)
                    };

                    // Snapshot into a signal so RawChart can be reactive
                    let opt_sig = Signal::derive(move || {
                        let s2 = scripts.get();
                        let r2 = results.get();
                        let items: Vec<(String, i64, i64, i64)> = s2.iter().filter_map(|sc| {
                            let res = r2.iter().find(|tr| tr.script_id == sc.id)?;
                            Some((sc.control_ref.clone(), res.passed_count, res.failed_count, res.error_count))
                        }).collect();
                        let refs: Vec<(&str, i64, i64, i64)> = items.iter()
                            .map(|(r, p, f, e)| (r.as_str(), *p, *f, *e))
                            .collect();
                        build_summary_option(&refs)
                    });

                    Some(view! {
                        <div style="background:var(--w-bg-2);border-radius:6px;\
                                    padding:10px 12px;margin-bottom:14px">
                            <div style="font-size:11px;font-weight:600;color:var(--w-text-3);\
                                        text-transform:uppercase;letter-spacing:0.05em;margin-bottom:6px">
                                "Results overview"
                            </div>
                            <RawChart chart_id="vr-step5-overview".to_string()
                                option_json=opt_sig height=160 />
                        </div>
                    })
                }}

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
                                                let pop    = stmt["population_size"].as_u64().unwrap_or(0);
                                                let rows: Vec<serde_json::Value> = stmt["selected_indices"]
                                                    .as_array().cloned().unwrap_or_default();
                                                let sel = rows.len();

                                                // Build a distribution chart if we have rows
                                                let chart_view = if !rows.is_empty() {
                                                    let best_col = pick_sample_column(&rows);
                                                    if let Some(col) = best_col {
                                                        let dist_pts: Vec<DistPoint> = sample_distribution(&rows, &col);
                                                        let col_label  = col.clone();
                                                        let chart_id   = format!("vr-s5-smp-{}-{}", sid6, stmt["index"].as_u64().unwrap_or(0));
                                                        let dist_sig   = Signal::derive({
                                                            let pts = dist_pts.clone();
                                                            move || pts.clone()
                                                        });
                                                        let tab_sig    = RwSignal::new(ChartTab::Bar);
                                                        Some(view! {
                                                            <div style="margin-top:8px;border:0.5px solid var(--w-border);\
                                                                        border-radius:4px;padding:8px 10px">
                                                                <div style="display:flex;align-items:center;gap:8px;margin-bottom:6px">
                                                                    <span style="font-size:11px;color:var(--w-text-3)">
                                                                        {format!("Distribution: {col_label}")}
                                                                    </span>
                                                                    <div style="margin-left:auto;display:flex;gap:4px">
                                                                        {[ChartTab::Bar, ChartTab::Pie, ChartTab::Table].into_iter().map(|t| {
                                                                            view! {
                                                                                <button
                                                                                    on:click=move |_| tab_sig.set(t)
                                                                                    style=move || {
                                                                                        let active = tab_sig.get() == t;
                                                                                        format!(
                                                                                            "font-size:10px;padding:2px 7px;\
                                                                                             border-radius:3px;cursor:pointer;\
                                                                                             border:1px solid var(--w-border);{}",
                                                                                            if active { "background:var(--w-accent);color:#fff" }
                                                                                            else      { "background:var(--w-bg);color:var(--w-text-3)" }
                                                                                        )
                                                                                    }>
                                                                                    {t.label()}
                                                                                </button>
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                </div>
                                                                {move || {
                                                                    if tab_sig.get() == ChartTab::Table {
                                                                        let d = dist_sig.get();
                                                                        let total: usize = d.iter().map(|p| p.count).sum();
                                                                        view! {
                                                                            <div style="overflow-x:auto;max-height:180px;overflow-y:auto">
                                                                                <table style="width:100%;border-collapse:collapse;font-size:11px">
                                                                                    <thead><tr>
                                                                                        <th style="text-align:left;padding:3px 6px;border-bottom:1px solid var(--w-border);color:var(--w-text-3)">"Value"</th>
                                                                                        <th style="text-align:right;padding:3px 6px;border-bottom:1px solid var(--w-border);color:var(--w-text-3)">"Count"</th>
                                                                                        <th style="text-align:right;padding:3px 6px;border-bottom:1px solid var(--w-border);color:var(--w-text-3)">"%"</th>
                                                                                    </tr></thead>
                                                                                    <tbody>
                                                                                        {d.into_iter().map(|pt| {
                                                                                            let pct = if total > 0 { format!("{:.1}", pt.count as f64 / total as f64 * 100.0) } else { "0".into() };
                                                                                            view! {
                                                                                                <tr>
                                                                                                    <td style="padding:3px 6px;border-bottom:0.5px solid var(--w-border);color:var(--w-text-2)">{pt.value}</td>
                                                                                                    <td style="padding:3px 6px;text-align:right;font-family:monospace;border-bottom:0.5px solid var(--w-border)">{pt.count}</td>
                                                                                                    <td style="padding:3px 6px;text-align:right;color:var(--w-text-3);border-bottom:0.5px solid var(--w-border)">{pct}"%"</td>
                                                                                                </tr>
                                                                                            }
                                                                                        }).collect_view()}
                                                                                    </tbody>
                                                                                </table>
                                                                            </div>
                                                                        }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <EChart chart_id=chart_id.clone()
                                                                                tab=Signal::derive(move || tab_sig.get())
                                                                                data=dist_sig />
                                                                        }.into_any()
                                                                    }
                                                                }}
                                                            </div>
                                                        }.into_any())
                                                    } else { None }
                                                } else { None };

                                                view! {
                                                    <div>
                                                        <div class="s5-stmt-row s5-stmt-sample">
                                                            <span class="s5-stmt-icon">"◎"</span>
                                                            <span class="s5-stmt-label">{method}</span>
                                                            <span class="s5-stmt-values">{format!("{sel} selected from {pop} items")}</span>
                                                        </div>
                                                        {chart_view}
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
                    <GhostButton label="⬇ Export Debug"
                        disabled=Signal::derive(move || results.get().is_empty())
                        on_click=Callback::new(move |()| {
                            export_debug(&scripts.get_untracked(), &results.get_untracked());
                        }) />
                    <GhostButton label="Findings →"
                        disabled=Signal::derive(move || results.get().is_empty())
                        on_click=Callback::new(move |()| audit_ui_step.set(8)) />
                </div>
            </div>

        </div>
    }
}
