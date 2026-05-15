use crate::projects::step5a::chart::{build_dsl_chart_option, RawChart};
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

            let failed_rows: Vec<Vec<(String, String)>> = r["failed_rows"]
                .as_array()
                .map(|arr| {
                    arr.iter()
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
                        .collect()
                })
                .unwrap_or_default();

            let cols: Vec<String> = failed_rows
                .first()
                .map(|r| r.iter().map(|(k, _)| k.clone()).collect())
                .unwrap_or_default();

            view! {
                <div class="ide-result-assert-wrap">
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
                    {(!cols.is_empty()).then(|| view! {
                        <div class="ide-sample-table-wrap ide-failures-table">
                            <table class="ide-sample-table">
                                <thead>
                                    <tr>
                                        {cols.iter().map(|c| view! {
                                            <th>{c.clone()}</th>
                                        }).collect_view()}
                                    </tr>
                                </thead>
                                <tbody>
                                    {failed_rows.into_iter().map(|row| view! {
                                        <tr>
                                            {row.into_iter().map(|(_, v)| view! {
                                                <td>{v}</td>
                                            }).collect_view()}
                                        </tr>
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    })}
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

        "show_rows" => {
            let lbl = r["label"].as_str().map(|s| s.to_string());
            let table = r["table"].as_str().unwrap_or("").to_string();
            let total = r["total"].as_u64().unwrap_or(0);

            let rows_val = r["rows"].as_array().cloned().unwrap_or_default();
            let show_rows: Vec<Vec<(String, String)>> = rows_val
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
            let cols: Vec<String> = show_rows
                .first()
                .map(|r| r.iter().map(|(k, _)| k.clone()).collect())
                .unwrap_or_default();

            view! {
                <div class="ide-result ide-result--show-rows">
                    <span class="ide-result-icon">
                        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                            <rect x="1" y="1" width="11" height="11" rx="1.5"
                                stroke="currentColor" stroke-width="1.1"/>
                            <path d="M1 4.5h11M1 7.5h11"
                                stroke="currentColor" stroke-width="1.1"/>
                        </svg>
                    </span>
                    <div class="ide-result-body" style="flex:1;min-width:0">
                        <div class="ide-result-sample-head">
                            {lbl.map(|l| view! {
                                <span class="ide-result-label">{l}</span>
                            })}
                            <span class="ide-result-expr">
                                {format!("{table} — {total} row{}", if total == 1 { "" } else { "s" })}
                            </span>
                        </div>
                        {(!cols.is_empty()).then(|| view! {
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
                                        {show_rows.into_iter().map(|row| view! {
                                            <tr>
                                                {row.into_iter().map(|(_, v)| view! {
                                                    <td>{v}</td>
                                                }).collect_view()}
                                            </tr>
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        })}
                    </div>
                </div>
            }.into_any()
        }

        _ => view! { <div /> }.into_any(),
    }
}

// ── Pop-out window for testing DSL results ──────────────────────────────────

/// Build self-contained HTML for a pop-out results window.
///
/// Delegates to the shared page renderer in the core `vinrouge` crate.
pub fn build_results_html(results: &[serde_json::Value]) -> String {
    vinrouge::dsl::html::render_html_page(results)
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
