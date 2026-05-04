use crate::step5a::chart::{RawChart, build_dsl_chart_option};
use leptos::prelude::*;

/// Render a single DSL result item (assert, chart, section, etc.) recursively.
pub fn render_dsl_result(idx: usize, r: serde_json::Value) -> AnyView {
    let kind = r["kind"].as_str().unwrap_or("").to_string();
    match kind.as_str() {
        "assert" => {
            let ok   = r["passed"].as_bool().unwrap_or(false);
            let lbl  = r["label"].as_str().map(|s| s.to_string());
            let lhs  = r["lhs_value"].as_str().unwrap_or("?").to_string();
            let rhs  = r["rhs_value"].as_str().unwrap_or("?").to_string();
            let op   = r["op"].as_str().unwrap_or("=").to_string();
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
            let method    = r["method"].as_str().unwrap_or("SAMPLE").to_string();
            let pop_size  = r["population_size"].as_u64().unwrap_or(0);
            let sel_count = r["selected_count"].as_u64().unwrap_or(0);
            let pct       = if pop_size > 0 { sel_count as f64 / pop_size as f64 * 100.0 } else { 0.0 };

            let rows_val = r["selected_indices"].as_array().cloned().unwrap_or_default();
            let sample_rows: Vec<Vec<(String, String)>> = rows_val.iter()
                .filter_map(|rv| rv.as_object().map(|obj| {
                    let mut pairs: Vec<(String, String)> = obj.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())))
                        .collect();
                    pairs.sort_by(|a, b| a.0.cmp(&b.0));
                    pairs
                }))
                .collect();
            let cols: Vec<String> = sample_rows.first()
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
            }.into_any()
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
            }.into_any()
        }

        "relation" => {
            let from = r["from"].as_str().unwrap_or("").to_string();
            let to   = r["to"].as_str().unwrap_or("").to_string();
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
            let labels: Vec<String> = r["labels"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let values: Vec<String> = r["values"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let option = build_dsl_chart_option(&ctype, &labels, &values);
            let opt_sig: Signal<String> = create_rw_signal(option).into();
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
            }.into_any()
        }

        "screen" => {
            let title = r["title"].as_str().unwrap_or("Screen").to_string();
            let screen_charts: Vec<serde_json::Value> = r["charts"].as_array()
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
                                let opt_sig: Signal<String> = create_rw_signal(option).into();
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
            let expanded = create_rw_signal(true);
            let title = r["title"].as_str().unwrap_or("Section").to_string();
            let passed = r["passed"].as_u64().unwrap_or(0);
            let failed = r["failed"].as_u64().unwrap_or(0);
            let errors = r["errors"].as_u64().unwrap_or(0);
            let inner: Vec<serde_json::Value> = r["results"].as_array()
                .map(|a| a.iter().cloned().collect())
                .unwrap_or_default();
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
                    {move || expanded.get().then(|| view! {
                        <div class="ide-section-body">
                            {inner.clone().into_iter().enumerate().map(|(i, child)| render_dsl_result(i, child)).collect::<Vec<_>>()}
                        </div>
                    })}
                </div>
            }.into_any()
        }

        _ => view! { <div /> }.into_any()
    }
}

/// Recursively count result kinds, drilling into sections.
pub fn count_results(results: &[serde_json::Value]) -> (usize, usize, usize, usize, usize, usize, usize) {
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
