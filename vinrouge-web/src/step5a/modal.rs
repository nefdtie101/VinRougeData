use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::ipc::tauri_invoke_args;
use crate::types::{DslScript, TestResult};

use super::chart::EChart;
use super::types::{ChartTab, DistPoint};

// ── FindingModal ──────────────────────────────────────────────────────────────

#[component]
pub fn FindingModal(
    script: DslScript,
    result: TestResult,
    on_close: Callback<()>,
) -> impl IntoView {
    let tab: RwSignal<ChartTab>         = RwSignal::new(ChartTab::Bar);
    let dist: RwSignal<Vec<DistPoint>>  = RwSignal::new(vec![]);
    let loading: RwSignal<bool>         = RwSignal::new(false);
    let err_sig: RwSignal<Option<String>> = RwSignal::new(None);

    // Collect distinct source columns from failed assertions.
    let all_cols: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        result.results.iter()
            .filter(|s| {
                s["kind"].as_str() == Some("assert")
                && !s["passed"].as_bool().unwrap_or(true)
                && s.get("source_col").and_then(|v| v.as_str()).is_some()
            })
            .filter_map(|s| s["source_col"].as_str().map(String::from))
            .filter(|c| seen.insert(c.clone()))
            .collect()
    };

    let first_col = all_cols.first().cloned();
    let active_col: RwSignal<Option<String>> = RwSignal::new(first_col);

    // Load distribution whenever active_col changes.
    Effect::new(move |_| {
        let col = active_col.get();
        let Some(col) = col else {
            dist.set(vec![]);
            return;
        };
        loading.set(true);
        err_sig.set(None);
        spawn_local(async move {
            match tauri_invoke_args::<Vec<DistPoint>>(
                "get_column_distribution",
                serde_json::json!({ "tableCol": col }),
            ).await {
                Ok(pts) => dist.set(pts),
                Err(e)  => err_sig.set(Some(e)),
            }
            loading.set(false);
        });
    });

    // Static data cloned for use inside closures.
    let label     = script.label.clone();
    let ctrl_ref  = script.control_ref.clone();
    let script_id = script.id.clone();

    // Pre-build the static failed-assertions list (rendered once, not reactive).
    let failed_rows: Vec<_> = result.results.iter()
        .filter(|s| {
            let k = s["kind"].as_str().unwrap_or("");
            (k == "assert" && !s["passed"].as_bool().unwrap_or(true)) || k == "error"
        })
        .map(|s| {
            let kind = s["kind"].as_str().unwrap_or("");
            if kind == "assert" {
                let lbl = s["label"].as_str().unwrap_or("assert").to_string();
                let lhs = s["lhs_value"].as_str().unwrap_or("?").to_string();
                let op  = s["op"].as_str().unwrap_or("=").to_string();
                let rhs = s["rhs_value"].as_str().unwrap_or("?").to_string();
                view! {
                    <div style="padding:6px 10px;background:rgba(224,92,92,0.08);
                                border-left:3px solid #e05c5c;border-radius:3px;
                                font-size:12px;color:var(--w-text-1)">
                        <span style="font-weight:600">{lbl}</span>
                        <span style="color:#e05c5c;margin-left:8px">
                            {format!("actual {lhs} {op} {rhs}")}
                        </span>
                    </div>
                }.into_any()
            } else {
                let err = s["error"].as_str().unwrap_or("error").to_string();
                view! {
                    <div style="padding:6px 10px;background:rgba(240,165,0,0.08);
                                border-left:3px solid #f0a500;border-radius:3px;
                                font-size:12px;color:#f0a500">
                        {err}
                    </div>
                }.into_any()
            }
        })
        .collect();

    let dist_sig  = Signal::derive(move || dist.get());
    let tab_sig   = Signal::derive(move || tab.get());
    let has_cols  = !all_cols.is_empty();
    let multi_col = all_cols.len() > 1;
    let cols_for_select = all_cols.clone();

    view! {
        // Backdrop
        <div
            style="position:fixed;inset:0;z-index:900;background:#0d0a0b;
                   display:flex;align-items:center;justify-content:center"
            on:click=move |_| on_close.run(())>

            // Panel
            <div
                style="position:relative;background:var(--w-bg-2);border:1px solid var(--w-border-2);\
                       border-radius:8px;width:min(700px,92vw);max-height:88vh;\
                       display:flex;flex-direction:column;overflow:hidden"
                on:click=|e| e.stop_propagation()>

                // ── Header ────────────────────────────────────────────────────
                <div style="padding:14px 16px;border-bottom:0.5px solid var(--w-border);
                            display:flex;align-items:flex-start;gap:10px">
                    <div style="flex:1;min-width:0">
                        <div style="font-size:13px;font-weight:700;color:#e05c5c;
                                    white-space:nowrap;overflow:hidden;text-overflow:ellipsis">
                            {format!("FINDING — {label}")}
                        </div>
                        <div style="font-size:11px;color:var(--w-text-3);margin-top:2px">
                            {format!("Control {ctrl_ref}")}
                        </div>
                    </div>
                    <button
                        style="background:none;border:none;color:var(--w-text-3);
                               font-size:16px;cursor:pointer;padding:2px 6px;flex-shrink:0"
                        on:click=move |_| on_close.run(())>
                        "✕"
                    </button>
                </div>

                // ── Body ──────────────────────────────────────────────────────
                <div style="flex:1;overflow-y:auto;padding:14px;display:flex;
                            flex-direction:column;gap:14px">

                    // Static: failed assertion rows
                    <div style="display:flex;flex-direction:column;gap:6px">
                        {failed_rows}
                    </div>

                    // Reactive: chart section
                    {move || has_cols.then(|| {
                        let chart_id = format!("vr-chart-{script_id}");
                        view! {
                            <div>
                                // Tab/column controls
                                <div style="display:flex;align-items:center;gap:8px;
                                            margin-bottom:10px;flex-wrap:wrap">

                                    // Column selector — only when multiple columns
                                    {multi_col.then(|| {
                                        let cols = cols_for_select.clone();
                                        view! {
                                            <select
                                                style="font-size:11px;background:var(--w-bg);
                                                       color:var(--w-text-2);border:1px solid var(--w-border);
                                                       border-radius:4px;padding:3px 6px"
                                                on:change=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    active_col.set(if val.is_empty() { None } else { Some(val) });
                                                }>
                                                {cols.into_iter().map(|c| {
                                                    let c2 = c.clone();
                                                    let c3 = c.clone();
                                                    view! {
                                                        <option value=c2.clone()
                                                            selected=move || active_col.get().as_deref() == Some(c2.as_str())>
                                                            {c3}
                                                        </option>
                                                    }
                                                }).collect_view()}
                                            </select>
                                        }
                                    })}

                                    <span style="font-size:11px;color:var(--w-text-3);flex:1">
                                        {move || active_col.get()
                                            .map(|c| format!("Distribution: {c}"))
                                            .unwrap_or_default()}
                                    </span>

                                    // Chart type tabs
                                    {[ChartTab::Bar, ChartTab::Pie, ChartTab::Table].into_iter().map(|t| {
                                        view! {
                                            <button
                                                style=move || {
                                                    let active = tab.get() == t;
                                                    format!(
                                                        "font-size:11px;padding:3px 10px;border-radius:4px;\
                                                         cursor:pointer;border:1px solid var(--w-border);{}",
                                                        if active {
                                                            "background:var(--w-accent);color:#fff;"
                                                        } else {
                                                            "background:var(--w-bg);color:var(--w-text-3);"
                                                        }
                                                    )
                                                }
                                                on:click=move |_| tab.set(t)>
                                                {t.label()}
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>

                                // Chart / table / spinner
                                {move || {
                                    if loading.get() {
                                        view! {
                                            <div style="display:flex;align-items:center;gap:6px;
                                                        font-size:12px;color:var(--w-text-3);padding:20px 0">
                                                <crate::components::Spinner size=12 />
                                                " Loading…"
                                            </div>
                                        }.into_any()
                                    } else if let Some(e) = err_sig.get() {
                                        view! {
                                            <div style="font-size:12px;color:#e05c5c;padding:8px 0">{e}</div>
                                        }.into_any()
                                    } else if tab.get() == ChartTab::Table {
                                        view! {
                                            <div style="overflow-x:auto;max-height:220px;overflow-y:auto">
                                                <table style="width:100%;border-collapse:collapse;font-size:12px">
                                                    <thead>
                                                        <tr>
                                                            <th style="text-align:left;padding:5px 8px;\
                                                                       border-bottom:1px solid var(--w-border);\
                                                                       color:var(--w-text-3);font-size:11px">"Value"</th>
                                                            <th style="text-align:right;padding:5px 8px;\
                                                                       border-bottom:1px solid var(--w-border);\
                                                                       color:var(--w-text-3);font-size:11px">"Count"</th>
                                                            <th style="text-align:right;padding:5px 8px;\
                                                                       border-bottom:1px solid var(--w-border);\
                                                                       color:var(--w-text-3);font-size:11px">"%"</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {move || {
                                                            let d = dist.get();
                                                            let total: usize = d.iter().map(|p| p.count).sum();
                                                            d.into_iter().map(|pt| {
                                                                let pct = if total > 0 {
                                                                    format!("{:.1}", pt.count as f64 / total as f64 * 100.0)
                                                                } else {
                                                                    "0.0".to_string()
                                                                };
                                                                view! {
                                                                    <tr>
                                                                        <td style="padding:4px 8px;color:var(--w-text-2);\
                                                                                   border-bottom:0.5px solid var(--w-border)">
                                                                            {pt.value}
                                                                        </td>
                                                                        <td style="padding:4px 8px;text-align:right;\
                                                                                   font-family:monospace;\
                                                                                   border-bottom:0.5px solid var(--w-border)">
                                                                            {pt.count}
                                                                        </td>
                                                                        <td style="padding:4px 8px;text-align:right;\
                                                                                   color:var(--w-text-3);\
                                                                                   border-bottom:0.5px solid var(--w-border)">
                                                                            {pct}"%"
                                                                        </td>
                                                                    </tr>
                                                                }
                                                            }).collect_view()
                                                        }}
                                                    </tbody>
                                                </table>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <EChart chart_id=chart_id.clone() tab=tab_sig data=dist_sig />
                                        }.into_any()
                                    }
                                }}
                            </div>
                        }
                    })}

                    // Hint when no column ref found
                    {(!has_cols).then(|| view! {
                        <div style="font-size:12px;color:var(--w-text-3);padding:8px 0;font-style:italic">
                            "No column reference in failed assertions — chart unavailable."
                        </div>
                    })}

                </div>
            </div>
        </div>
    }
}
