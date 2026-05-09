use crate::step5a::chart::{build_dsl_chart_option, RawChart};
use leptos::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

/// Render a single DSL result item (assert, chart, section, etc.) recursively.
pub fn render_dsl_result(idx: usize, r: serde_json::Value) -> AnyView {
    let kind = r["kind"].as_str().unwrap_or("").to_string();
    match kind.as_str() {
        "assert" => {
            let ok = r["passed"].as_bool().unwrap_or(false);
            let lbl = r["label"].as_str().map(|s| s.to_string());
            let lhs = r["lhs_value"].as_str().unwrap_or("?").to_string();
            let rhs = r["rhs_value"].as_str().unwrap_or("?").to_string();
            let op = r["op"].as_str().unwrap_or("=").to_string();
            view! {
                <div class=if ok { "ide-result ide-result--pass" } else { "ide-result ide-result--fail" }>
                    <span class="ide-result-icon">
                        {if ok {
                            view! {
                                <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                                    <path d="M2 6.5l3.5 3.5 5.5-6"
                                        stroke="currentColor" stroke-width="1.6"
                                        stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                                    <path d="M2.5 2.5l8 8M10.5 2.5l-8 8"
                                        stroke="currentColor" stroke-width="1.6"
                                        stroke-linecap="round"/>
                                </svg>
                            }.into_any()
                        }}
                    </span>
                    <div class="ide-result-body">
                        {lbl.map(|l| view! {
                            <span class="ide-result-label">{l}</span>
                        })}
                        <span class="ide-result-expr">
                            <span class="ide-result-val">{lhs}</span>
                            <span class="ide-result-op">{op}</span>
                            <span class="ide-result-val">{rhs}</span>
                        </span>
                    </div>
                </div>
            }.into_any()
        }

        "sample" => {
            let method = r["method"].as_str().unwrap_or("SAMPLE").to_string();
            let pop_size = r["population_size"].as_u64().unwrap_or(0);
            let sel_count = r["selected_count"].as_u64().unwrap_or(0);
            let pct = if pop_size > 0 {
                sel_count as f64 / pop_size as f64 * 100.0
            } else {
                0.0
            };

            let rows_val = r["selected_indices"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let sample_rows: Vec<Vec<(String, String)>> = rows_val
                .iter()
                .filter_map(|rv| {
                    rv.as_object().map(|obj| {
                        let mut pairs: Vec<(String, String)> = obj
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    v.as_str()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| v.to_string()),
                                )
                            })
                            .collect();
                        pairs.sort_by(|a, b| a.0.cmp(&b.0));
                        pairs
                    })
                })
                .collect();
            let cols: Vec<String> = sample_rows
                .first()
                .map(|r| r.iter().map(|(k, _)| k.clone()).collect())
                .unwrap_or_default();

            view! {
                <div class="ide-result ide-result--sample">
                    <span class="ide-result-icon">
                        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                            <rect x="1" y="1" width="11" height="11" rx="1.5"
                                stroke="currentColor" stroke-width="1.1"/>
                            <path d="M1 4.5h11M4.5 4.5v7"
                                stroke="currentColor" stroke-width="1.1"/>
                        </svg>
                    </span>
                    <div class="ide-result-body" style="flex:1;min-width:0">
                        <div class="ide-result-sample-head">
                            <span class="ide-result-label">{method.to_uppercase()}</span>
                            <span class="ide-result-expr">
                                {format!("{sel_count} of {pop_size} rows  ({pct:.1}%)")}
                            </span>
                        </div>
                        {(!cols.is_empty()).then(|| {
                            let preview_rows = sample_rows.into_iter().take(8).collect::<Vec<_>>();
                            let remaining = sel_count as usize - preview_rows.len().min(sel_count as usize);
                            view! {
                                <div class="ide-sample-table-wrap">
                                    <table class="ide-sample-table">
                                        <thead>
                                            <tr>
                                                {cols.iter().map(|c| view! {
                                                    <th>{c.clone()}</th>
                                                }).collect_view()}
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {preview_rows.into_iter().map(|row| view! {
                                                <tr>
                                                    {row.into_iter().map(|(_, v)| view! {
                                                        <td>{v}</td>
                                                    }).collect_view()}
                                                </tr>
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                    {(remaining > 0).then(|| view! {
                                        <div class="ide-sample-more">
                                            {format!("+ {remaining} more rows")}
                                        </div>
                                    })}
                                </div>
                            }
                        })}
                    </div>
                </div>
            }.into_any()
        }

        "error" => {
            let msg = r["error"].as_str().unwrap_or("unknown error").to_string();
            view! {
                <div class="ide-result ide-result--error">
                    <span class="ide-result-icon">
                        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                            <path d="M6.5 1.5L11.5 10.5H1.5L6.5 1.5Z"
                                stroke="currentColor" stroke-width="1.2"
                                stroke-linejoin="round"/>
                            <path d="M6.5 5v3M6.5 9.5v.5"
                                stroke="currentColor" stroke-width="1.3"
                                stroke-linecap="round"/>
                        </svg>
                    </span>
                    <div class="ide-result-body">
                        <span class="ide-result-msg">{msg}</span>
                    </div>
                </div>
            }
            .into_any()
        }

        "value" => {
            let val = r["value"].as_str().unwrap_or("").to_string();
            view! {
                <div class="ide-result ide-result--value">
                    <span class="ide-result-icon">
                        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                            <path d="M2 6.5h9M7.5 3l3.5 3.5L7.5 10"
                                stroke="currentColor" stroke-width="1.3"
                                stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                    </span>
                    <div class="ide-result-body">
                        <span class="ide-result-expr">{val}</span>
                    </div>
                </div>
            }
            .into_any()
        }

        "relation" => {
            let from = r["from"].as_str().unwrap_or("").to_string();
            let to = r["to"].as_str().unwrap_or("").to_string();
            view! {
                <div class="ide-result ide-result--relation">
                    <span class="ide-result-icon">
                        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                            <circle cx="3" cy="6.5" r="1.5" stroke="currentColor" stroke-width="1.1"/>
                            <circle cx="10" cy="6.5" r="1.5" stroke="currentColor" stroke-width="1.1"/>
                            <path d="M4.5 6.5h3" stroke="currentColor" stroke-width="1.1"/>
                        </svg>
                    </span>
                    <div class="ide-result-body">
                        <span class="ide-result-expr">
                            <span class="ide-result-val">{from}</span>
                            <span class="ide-result-op">"→"</span>
                            <span class="ide-result-val">{to}</span>
                        </span>
                    </div>
                </div>
            }.into_any()
        }

        "chart" => {
            let lbl = r["label"].as_str().map(|s| s.to_string());
            let ctype = r["chart_type"].as_str().unwrap_or("bar").to_string();
            let labels: Vec<String> = r["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let values: Vec<String> = r["values"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let option = build_dsl_chart_option(&ctype, &labels, &values);
            let opt_sig: Signal<String> = RwSignal::new(option).into();
            let chart_id = format!("dsl-chart-{idx}");
            view! {
                <div class="ide-result ide-result--chart">
                    <div class="ide-result-body" style="flex:1;min-width:0">
                        {lbl.map(|l| view! {
                            <span class="ide-result-label">{l}</span>
                        })}
                        <RawChart chart_id=chart_id option_json=opt_sig height=220 />
                    </div>
                </div>
            }
            .into_any()
        }

        "screen" => {
            let title = r["title"].as_str().unwrap_or("Screen").to_string();
            let screen_charts: Vec<serde_json::Value> = r["charts"]
                .as_array()
                .map(|a| a.iter().cloned().collect())
                .unwrap_or_default();
            view! {
                <div class="ide-result ide-result--screen">
                    <div class="ide-result-body" style="flex:1;min-width:0">
                        <span class="ide-result-label">{title}</span>
                        <div class="ide-screen-grid" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:12px;margin-top:8px;">
                            {screen_charts.into_iter().enumerate().map(|(cidx, c)| {
                                let clbl = c["label"].as_str().map(|s| s.to_string());
                                let ctype = c["chart_type"].as_str().unwrap_or("bar").to_string();
                                let labels: Vec<String> = c["labels"].as_array()
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                    .unwrap_or_default();
                                let values: Vec<String> = c["values"].as_array()
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                    .unwrap_or_default();
                                let option = build_dsl_chart_option(&ctype, &labels, &values);
                                let opt_sig: Signal<String> = RwSignal::new(option).into();
                                let chart_id = format!("dsl-screen-{idx}-{cidx}");
                                view! {
                                    <div>
                                        {clbl.map(|l| view! {
                                            <span class="ide-result-label" style="font-size:11px">{l}</span>
                                        })}
                                        <RawChart chart_id=chart_id option_json=opt_sig height=200 />
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                </div>
            }.into_any()
        }

        "section" => {
            let expanded = RwSignal::new(true);
            let title = r["title"].as_str().unwrap_or("Section").to_string();
            let passed = r["passed"].as_u64().unwrap_or(0);
            let failed = r["failed"].as_u64().unwrap_or(0);
            let errors = r["errors"].as_u64().unwrap_or(0);
            let inner: Vec<serde_json::Value> = r["results"]
                .as_array()
                .map(|a| a.iter().cloned().collect())
                .unwrap_or_default();
            let inner_for_body = inner.clone();
            view! {
                <div class="ide-result ide-result--section">
                    <div class="ide-section-header" on:click=move |_| expanded.update(|e| *e = !*e)>
                        <span class="ide-result-label">{title}</span>
                        <span class="ide-section-badges">
                            {(passed > 0).then(|| view! {
                                <span class="ide-sum ide-sum--pass">{format!("{passed} passed")}</span>
                            })}
                            {(failed > 0).then(|| view! {
                                <span class="ide-sum ide-sum--fail">{format!("{failed} failed")}</span>
                            })}
                            {(errors > 0).then(|| view! {
                                <span class="ide-sum ide-sum--error">{format!("{errors} error{}", if errors == 1 { "" } else { "s" })}</span>
                            })}
                        </span>
                        <span class="ide-section-chevron">
                            {move || if expanded.get() {
                                view! { <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M2 4.5l4 4 4-4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                            } else {
                                view! { <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M4.5 2l4 4-4 4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                            }}
                        </span>
                    </div>
                    {move || expanded.get().then({
                        let inner = inner_for_body.clone();
                        move || view! {
                            <div class="ide-section-body">
                                {inner.into_iter().enumerate().map(|(i, child)| render_dsl_result(i, child)).collect::<Vec<_>>()}
                            </div>
                        }
                    })}
                </div>
            }.into_any()
        }

        "schema" => {
            let tables: Vec<serde_json::Value> = r["tables"]
                .as_array()
                .map(|a| a.iter().cloned().collect())
                .unwrap_or_default();
            let table_count = tables.len();
            let expanded = RwSignal::new(true);

            view! {
                <div class="ide-result ide-result--schema">
                    <div class="ide-section-header" on:click=move |_| expanded.update(|e| *e = !*e)>
                        <span class="ide-result-icon">
                            <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                                <rect x="1.5" y="1.5" width="10" height="10" rx="1.5"
                                    stroke="currentColor" stroke-width="1.1"/>
                                <path d="M1.5 5h10M5 1.5v10"
                                    stroke="currentColor" stroke-width="1.1"/>
                            </svg>
                        </span>
                        <span class="ide-result-label">
                            {format!("Schema — {} table{}", table_count, if table_count == 1 { "" } else { "s" })}
                        </span>
                        <span class="ide-section-chevron">
                            {move || if expanded.get() {
                                view! { <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M2 4.5l4 4 4-4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                            } else {
                                view! { <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M4.5 2l4 4-4 4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg> }.into_any()
                            }}
                        </span>
                    </div>
                    {move || expanded.get().then({
                        let tables = tables.clone();
                        move || view! {
                            <div class="ide-section-body">
                                {tables.into_iter().map(|t| {
                                    let name      = t["name"].as_str().unwrap_or("").to_string();
                                    let row_count = t["row_count"].as_u64().unwrap_or(0);
                                    let cols: Vec<(String, String)> = t["columns"].as_array()
                                        .map(|a| a.iter().map(|c| (
                                            c["name"].as_str().unwrap_or("").to_string(),
                                            c["type"].as_str().unwrap_or("text").to_string(),
                                        )).collect())
                                        .unwrap_or_default();
                                    view! {
                                        <div class="ide-schema-table">
                                            <div class="ide-schema-table-header">
                                                <span class="ide-schema-table-name">{name}</span>
                                                <span class="ide-schema-table-rows">
                                                    {format!("{} row{}", row_count, if row_count == 1 { "" } else { "s" })}
                                                </span>
                                            </div>
                                            <div class="ide-schema-cols">
                                                {cols.into_iter().map(|(col, ty)| {
                                                    let ty2 = ty.clone();
                                                    view! {
                                                        <span class="ide-schema-col">
                                                            <span class="ide-schema-col-name">{col}</span>
                                                            <span class=format!("ide-schema-col-type ide-schema-col-type--{ty2}")>{ty}</span>
                                                        </span>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }
                    })}
                </div>
            }.into_any()
        }

        _ => view! { <div /> }.into_any(),
    }
}

/// Recursively count result kinds, drilling into sections.
pub fn count_results(
    results: &[serde_json::Value],
) -> (usize, usize, usize, usize, usize, usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut samples = 0;
    let mut charts = 0;
    let mut screens = 0;
    let mut sections = 0;
    for r in results {
        match r["kind"].as_str() {
            Some("assert") => {
                if r["passed"].as_bool() == Some(true) {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            Some("error") => errors += 1,
            Some("sample") => samples += 1,
            Some("chart") => charts += 1,
            Some("screen") => screens += 1,
            Some("section") => {
                sections += 1;
                let inner = r["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let (p, f, e, sa, c, sc, se) = count_results(inner);
                passed += p;
                failed += f;
                errors += e;
                samples += sa;
                charts += c;
                screens += sc;
                sections += se;
            }
            _ => {}
        }
    }
    (passed, failed, errors, samples, charts, screens, sections)
}

// ── Pop-out window for testing DSL results ──────────────────────────────────

fn build_chart_option_json(chart_type: &str, labels: &[String], values: &[String]) -> String {
    build_dsl_chart_option(chart_type, labels, values)
}

fn result_to_html(r: &serde_json::Value, chart_idx: &mut usize) -> String {
    let kind = r["kind"].as_str().unwrap_or("");
    match kind {
        "assert" => {
            let ok = r["passed"].as_bool().unwrap_or(false);
            let lbl = r["label"].as_str().unwrap_or("");
            let lhs = r["lhs_value"].as_str().unwrap_or("?");
            let rhs = r["rhs_value"].as_str().unwrap_or("?");
            let op = r["op"].as_str().unwrap_or("=");
            let icon = if ok {
                "<svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2 6.5l3.5 3.5 5.5-6\" stroke=\"#4ade80\" stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>"
            } else {
                "<svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2.5 2.5l8 8M10.5 2.5l-8 8\" stroke=\"#f87171\" stroke-width=\"1.6\" stroke-linecap=\"round\"/></svg>"
            };
            let bg = if ok {
                "rgba(74,222,128,0.06)"
            } else {
                "rgba(248,113,113,0.06)"
            };
            let border = if ok {
                "rgba(74,222,128,0.15)"
            } else {
                "rgba(248,113,113,0.15)"
            };
            let icon_bg = if ok {
                "rgba(74,222,128,0.12)"
            } else {
                "rgba(248,113,113,0.12)"
            };
            let val_color = if ok { "#86efac" } else { "#fca5a5" };
            let mut h = String::new();
            h.push_str(&format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:{};border:0.5px solid {};margin-bottom:6px;\">",
                bg, border
            ));
            h.push_str(&format!(
                "<span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:{};\">{}</span>",
                icon_bg, icon
            ));
            h.push_str("<div style=\"min-width:0;\">");
            if !lbl.is_empty() {
                h.push_str(&format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{}</div>", lbl));
            }
            h.push_str(&format!(
                "<div style=\"font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\"><span style=\"color:{};\">{}</span> <span style=\"color:#8a6e74;font-size:11px;\">{}</span> <span style=\"color:{};\">{}</span></div>",
                val_color, lhs, op, val_color, rhs
            ));
            h.push_str("</div></div>");
            h
        }
        "sample" => {
            let method = r["method"].as_str().unwrap_or("SAMPLE");
            let pop = r["population_size"].as_u64().unwrap_or(0);
            let sel = r["selected_count"].as_u64().unwrap_or(0);
            let pct = if pop > 0 {
                sel as f64 / pop as f64 * 100.0
            } else {
                0.0
            };
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(96,165,250,0.06);border:0.5px solid rgba(96,165,250,0.15);margin-bottom:6px;\">\
                    <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(96,165,250,0.12);\">\
                        <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><rect x=\"1\" y=\"1\" width=\"11\" height=\"11\" rx=\"1.5\" stroke=\"#60a5fa\" stroke-width=\"1.1\"/><path d=\"M1 4.5h11M4.5 4.5v7\" stroke=\"#60a5fa\" stroke-width=\"1.1\"/></svg>\
                    </span>\
                    <div style=\"min-width:0;\">\
                        <div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{}</div>\
                        <div style=\"font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\">{} of {} rows ({:.1}%)</div>\
                    </div>\
                </div>",
                method, sel, pop, pct
            )
        }
        "error" => {
            let msg = r["error"].as_str().unwrap_or("unknown error");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(251,146,60,0.06);border:0.5px solid rgba(251,146,60,0.15);margin-bottom:6px;\">\
                    <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(251,146,60,0.12);\">\
                        <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M6.5 1.5L11.5 10.5H1.5L6.5 1.5Z\" stroke=\"#fb923c\" stroke-width=\"1.2\" stroke-linejoin=\"round\"/><path d=\"M6.5 5v3M6.5 9.5v.5\" stroke=\"#fb923c\" stroke-width=\"1.3\" stroke-linecap=\"round\"/></svg>\
                    </span>\
                    <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#fdba74;word-break:break-word;\">{}</div>\
                </div>",
                msg
            )
        }
        "value" => {
            let val = r["value"].as_str().unwrap_or("");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(192,132,252,0.06);border:0.5px solid rgba(192,132,252,0.15);margin-bottom:6px;\">\
                    <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(192,132,252,0.12);\">\
                        <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2 6.5h9M7.5 3l3.5 3.5L7.5 10\" stroke=\"#c084fc\" stroke-width=\"1.3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>\
                    </span>\
                    <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\">{}</div>\
                </div>",
                val
            )
        }
        "relation" => {
            let from = r["from"].as_str().unwrap_or("");
            let to = r["to"].as_str().unwrap_or("");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(163,163,163,0.05);border:0.5px solid rgba(163,163,163,0.12);margin-bottom:6px;\">\
                    <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(163,163,163,0.10);\">\
                        <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><circle cx=\"3\" cy=\"6.5\" r=\"1.5\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/><circle cx=\"10\" cy=\"6.5\" r=\"1.5\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/><path d=\"M4.5 6.5h3\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/></svg>\
                    </span>\
                    <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\"><span style=\"color:#aaa;\">{}</span> <span style=\"color:#666;\">→</span> <span style=\"color:#aaa;\">{}</span></div>\
                </div>",
                from, to
            )
        }
        "chart" => {
            let lbl = r["label"].as_str().unwrap_or("");
            let ctype = r["chart_type"].as_str().unwrap_or("bar");
            let labels: Vec<String> = r["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let values: Vec<String> = r["values"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let option = build_chart_option_json(ctype, &labels, &values);
            let id = format!("pop-chart-{}", *chart_idx);
            *chart_idx += 1;
            let mut h = String::new();
            h.push_str(&format!(
                "<div style=\"padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;\">"
            ));
            if !lbl.is_empty() {
                h.push_str(&format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:6px;\">{}</div>", lbl));
            }
            h.push_str(&format!(
                "<div id=\"{}\" data-option=\"{}\" style=\"width:100%;height:220px;\"></div>",
                id,
                html_escape(&option)
            ));
            h.push_str("</div>");
            h
        }
        "screen" => {
            let title = r["title"].as_str().unwrap_or("Screen");
            let charts: Vec<&serde_json::Value> = r["charts"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            let mut h = String::new();
            h.push_str(&format!(
                "<div style=\"padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;\">\
                 <div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:6px;\">{}</div>\
                 <div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:10px;\">",
                title
            ));
            for c in charts {
                let clbl = c["label"].as_str().unwrap_or("");
                let ctype = c["chart_type"].as_str().unwrap_or("bar");
                let labels: Vec<String> = c["labels"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let values: Vec<String> = c["values"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let option = build_chart_option_json(ctype, &labels, &values);
                let id = format!("pop-chart-{}", *chart_idx);
                *chart_idx += 1;
                h.push_str("<div style=\"padding:6px;border-radius:4px;background:rgba(255,255,255,0.02);border:0.5px solid #2a2024;\">");
                if !clbl.is_empty() {
                    h.push_str(&format!(
                        "<div style=\"font-size:10px;color:#c9a8ae;margin-bottom:4px;\">{}</div>",
                        clbl
                    ));
                }
                h.push_str(&format!(
                    "<div id=\"{}\" data-option=\"{}\" style=\"width:100%;height:180px;\"></div>",
                    id,
                    html_escape(&option)
                ));
                h.push_str("</div>");
            }
            h.push_str("</div></div>");
            h
        }
        "section" => {
            let title = r["title"].as_str().unwrap_or("Section");
            let passed = r["passed"].as_u64().unwrap_or(0);
            let failed = r["failed"].as_u64().unwrap_or(0);
            let errors = r["errors"].as_u64().unwrap_or(0);
            let inner: Vec<&serde_json::Value> = r["results"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            let mut badges = Vec::new();
            if passed > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(74,222,128,0.12);color:#4ade80;\">{} passed</span>", passed));
            }
            if failed > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(248,113,113,0.12);color:#f87171;\">{} failed</span>", failed));
            }
            if errors > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(251,146,60,0.12);color:#fb923c;\">{} error{}</span>", errors, if errors == 1 { "" } else { "s" }));
            }
            let badges_html = badges.join("");
            let inner_html: String = inner.iter().map(|c| result_to_html(c, chart_idx)).collect();
            let inner_id = format!(
                "sec-{:x}",
                title
                    .as_bytes()
                    .iter()
                    .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64))
            );
            format!(
                "<div style=\"border:0.5px solid #2a2024;border-radius:6px;background:rgba(19,14,16,0.6);overflow:hidden;margin-bottom:8px;\">\
                    <div style=\"display:flex;align-items:center;gap:10px;padding:8px 12px;background:rgba(255,255,255,0.03);cursor:pointer;user-select:none;\"\
                     onclick=\"var b=document.getElementById('{}');b.style.display=b.style.display==='none'?'block':'none';var c=this.querySelector('.chev');c.style.transform=b.style.display==='none'?'rotate(-90deg)':'rotate(0deg)';\">\
                        <span style=\"font-weight:600;font-size:13px;color:#d4c4c8;\">{}</span>\
                        <span style=\"display:flex;align-items:center;gap:6px;margin-left:auto;\">{}</span>\
                        <span class=\"chev\" style=\"color:#8a6e74;display:flex;align-items:center;transition:transform 0.15s;\">\
                            <svg width=\"12\" height=\"12\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 4.5l4 4 4-4\" stroke=\"currentColor\" stroke-width=\"1.2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>\
                        </span>\
                    </div>\
                    <div id=\"{}\" style=\"padding:8px 12px 12px;display:flex;flex-direction:column;gap:6px;\">{}</div>\
                </div>",
                inner_id, title, badges_html, inner_id, inner_html
            )
        }
        _ => String::new(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build self-contained HTML for a pop-out results window.
pub fn build_results_html(results: &[serde_json::Value]) -> String {
    let mut chart_idx = 0;
    let body: String = results
        .iter()
        .map(|r| result_to_html(r, &mut chart_idx))
        .collect();
    let (passed, failed, errors, samples, charts, screens, sections) = count_results(results);
    let mut summary = Vec::new();
    if passed > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(74,222,128,0.12);color:#4ade80;display:flex;align-items:center;gap:4px;\"><svg width=\"10\" height=\"10\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 6l3 3 5-5\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>{} passed</span>", passed));
    }
    if failed > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(248,113,113,0.12);color:#f87171;display:flex;align-items:center;gap:4px;\"><svg width=\"10\" height=\"10\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 2l8 8M10 2l-8 8\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linecap=\"round\"/></svg>{} failed</span>", failed));
    }
    if errors > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(251,146,60,0.12);color:#fb923c;\">{} error{}</span>", errors, if errors == 1 { "" } else { "s" }));
    }
    if samples > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(96,165,250,0.12);color:#60a5fa;\">{} sample{}</span>", samples, if samples == 1 { "" } else { "s" }));
    }
    if charts > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} chart{}</span>", charts, if charts == 1 { "" } else { "s" }));
    }
    if screens > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} screen{}</span>", screens, if screens == 1 { "" } else { "s" }));
    }
    if sections > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} section{}</span>", sections, if sections == 1 { "" } else { "s" }));
    }
    let summary_html = summary.join("");

    format!(
        r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>DSL Results</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
        *{{box-sizing:border-box}}
        body{{margin:0;padding:0;background:#141414;color:#d4c4c8;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;min-height:100vh;}}
        .wrap{{max-width:900px;margin:0 auto;padding:20px 16px 40px;}}
        .topbar{{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:16px;padding-bottom:12px;border-bottom:0.5px solid #2a2024;}}
        .title{{font-size:16px;font-weight:600;color:#d4c4c8;margin:0;}}
</style></head><body>
<div class="wrap">
    <div class="topbar">
        <h1 class="title">DSL Results</h1>
        <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-left:auto;">{}</div>
    </div>
    {}
</div>
<script>
document.addEventListener('DOMContentLoaded', function(){{
    var charts = document.querySelectorAll('[data-option]');
    for (var i = 0; i < charts.length; i++) {{
        var el = charts[i];
        var optStr = el.getAttribute('data-option');
        if (!optStr) continue;
        try {{
            var opt = JSON.parse(optStr.replace(/&quot;/g, '"').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>'));
            var chart = echarts.init(el, 'dark', {{ renderer: 'canvas' }});
            chart.setOption(opt);
        }} catch (e) {{
            console.error('Chart init failed', e);
            el.textContent = 'Chart error: ' + e.message;
        }}
    }}
}});
</script>
</body></html>"##,
        summary_html, body
    )
}

/// Open DSL results in a new browser window.
pub fn open_results_window(results: &[serde_json::Value]) {
    if results.is_empty() {
        return;
    }
    let html = build_results_html(results);
    let blob =
        web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&html))).ok();
    let url = blob.and_then(|b| web_sys::Url::create_object_url_with_blob(&b).ok());
    if let Some(u) = url {
        if let Some(w) = web_sys::window() {
            let _ = w.open_with_url_and_target(&u, "_blank");
        }
    }
}

/// Open DSL results in a pop-out, using a Tauri native window when available
/// or a browser tab otherwise.
pub fn open_results_window_async(results: Vec<serde_json::Value>) {
    if results.is_empty() {
        return;
    }

    if crate::ipc::is_tauri() {
        spawn_local(async move {
            let args = serde_json::json!({ "results": results });
            let _ = crate::ipc::tauri_invoke_args::<()>("open_dsl_results_window", args).await;
        });
        return;
    }

    open_results_window(&results);
}
