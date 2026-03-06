use leptos::*;
use std::rc::Rc;
use vinrouge::export::AnalysisResult;

#[component]
pub fn ResultsView(result: Rc<AnalysisResult>) -> impl IntoView {
    let (active_tab, set_tab) = create_signal("schema");

    let result_schema = result.clone();
    let result_profile = result.clone();
    let result_rel = result.clone();
    let result_recon = result.clone();
    let result_mv = result.clone();

    view! {
        <div class="results-panel">
            <nav class="tab-bar">
                <TabButton label="Schema" id="schema" active=active_tab set_tab=set_tab />
                <TabButton label="Data Profile" id="profile" active=active_tab set_tab=set_tab />
                <TabButton label="Relationships" id="relationships" active=active_tab set_tab=set_tab />
                <TabButton label="Reconciliation" id="reconciliation" active=active_tab set_tab=set_tab />
                <TabButton label="Multi-Value" id="multivalue" active=active_tab set_tab=set_tab />
            </nav>

            <div class="tab-content">
                {move || match active_tab.get() {
                    "schema" => view! { <SchemaTab result=result_schema.clone() /> }.into_view(),
                    "profile" => view! { <ProfileTab result=result_profile.clone() /> }.into_view(),
                    "relationships" => view! { <RelTab result=result_rel.clone() /> }.into_view(),
                    "reconciliation" => view! { <ReconTab result=result_recon.clone() /> }.into_view(),
                    "multivalue" => view! { <MvTab result=result_mv.clone() /> }.into_view(),
                    _ => view! { <div /> }.into_view(),
                }}
            </div>
        </div>
    }
}

#[component]
fn TabButton(
    label: &'static str,
    id: &'static str,
    active: ReadSignal<&'static str>,
    set_tab: WriteSignal<&'static str>,
) -> impl IntoView {
    view! {
        <button
            class=move || if active.get() == id { "tab-btn tab-btn--active" } else { "tab-btn" }
            on:click=move |_| set_tab.set(id)
        >
            {label}
        </button>
    }
}

// ── Schema tab ────────────────────────────────────────────────────────────────

#[component]
fn SchemaTab(result: Rc<AnalysisResult>) -> impl IntoView {
    let tables = result.tables.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Schema — " {tables.len()} " tables"</h2>
            {tables.into_iter().map(|table| {
                let cols = table.columns.clone();
                view! {
                    <details class="table-card" open=true>
                        <summary class="table-card__header">
                            <strong>{table.name.clone()}</strong>
                            <span class="badge">{cols.len()} " columns"</span>
                            {table.row_count.map(|r| view! {
                                <span class="badge badge--grey">{r} " rows"</span>
                            })}
                        </summary>
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>"Column"</th>
                                    <th>"Type"</th>
                                    <th>"Nulls"</th>
                                    <th>"Unique"</th>
                                    <th>"Sample values"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {cols.into_iter().map(|col| {
                                    let samples = col.sample_values.join(", ");
                                    view! {
                                        <tr>
                                            <td class="col-name">{col.name}</td>
                                            <td><code>{format!("{:?}", col.data_type)}</code></td>
                                            <td>{col.null_count.map(|n| n.to_string()).unwrap_or_default()}</td>
                                            <td>{col.unique_count.map(|n| n.to_string()).unwrap_or_default()}</td>
                                            <td class="samples">{samples}</td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    </details>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

// ── Data Profile tab ──────────────────────────────────────────────────────────

#[component]
fn ProfileTab(result: Rc<AnalysisResult>) -> impl IntoView {
    let profiles = result.data_profiles.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Data Profiles"</h2>
            {if profiles.is_empty() {
                view! { <p class="empty">"No data profiles available."</p> }.into_view()
            } else {
                profiles.into_iter().map(|p| {
                    let col_count = p.column_profiles.len();
                    view! {
                        <div class="profile-card">
                            <h3>{p.table_name.clone()}</h3>
                            <p>"Columns profiled: " {col_count}</p>
                            <table class="data-table">
                                <thead>
                                    <tr>
                                        <th>"Column"</th>
                                        <th>"Total"</th>
                                        <th>"Unique"</th>
                                        <th>"Nulls"</th>
                                        <th>"Distinct ratio"</th>
                                        <th>"Patterns"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {p.column_profiles.into_iter().map(|cp| {
                                        let ratio = format!("{:.1}%", cp.distinct_ratio * 100.0);
                                        let patterns = cp.data_patterns
                                            .iter()
                                            .map(|p| format!("{:?}", p))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        view! {
                                            <tr>
                                                <td>{cp.column_name}</td>
                                                <td>{cp.total_values}</td>
                                                <td>{cp.unique_values}</td>
                                                <td>{cp.null_count}</td>
                                                <td>{ratio}</td>
                                                <td class="samples">{patterns}</td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </div>
                    }
                }).collect::<Vec<_>>().into_view()
            }}
        </div>
    }
}

// ── Relationships tab ─────────────────────────────────────────────────────────

#[component]
fn RelTab(result: Rc<AnalysisResult>) -> impl IntoView {
    let rels = result.relationships.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Relationships — " {rels.len()}</h2>
            {if rels.is_empty() {
                view! { <p class="empty">"No relationships detected."</p> }.into_view()
            } else {
                view! {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>"From table"</th>
                                <th>"Column"</th>
                                <th>"→"</th>
                                <th>"To table"</th>
                                <th>"Column"</th>
                                <th>"Type"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rels.into_iter().map(|r| {
                                let rtype = format!("{:?}", r.relationship_type);
                                view! {
                                    <tr>
                                        <td>{r.from_table}</td>
                                        <td><code>{r.from_column}</code></td>
                                        <td>"→"</td>
                                        <td>{r.to_table}</td>
                                        <td><code>{r.to_column}</code></td>
                                        <td>{rtype}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                }.into_view()
            }}
        </div>
    }
}

// ── Reconciliation tab ────────────────────────────────────────────────────────

#[component]
fn ReconTab(result: Rc<AnalysisResult>) -> impl IntoView {
    let recons = result.reconciliation_results.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Reconciliation"</h2>
            {if recons.is_empty() {
                view! {
                    <p class="empty">
                        "Upload at least two files to see reconciliation results."
                    </p>
                }.into_view()
            } else {
                recons.into_iter().map(|r| {
                    let pct = format!("{:.1}%", r.match_percentage);
                    view! {
                        <div class="recon-card">
                            <h3>{r.source1_name.clone()} " vs " {r.source2_name.clone()}</h3>
                            <p>
                                "Match: " {pct}
                                "  ·  Source A: " {r.total_source1}
                                "  ·  Source B: " {r.total_source2}
                                "  ·  Matched: " {r.matches}
                                "  ·  Only in A: " {r.only_in_source1}
                                "  ·  Only in B: " {r.only_in_source2}
                            </p>
                            {if !r.field_mismatches.is_empty() {
                                view! {
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Key"</th>
                                                <th>"Column"</th>
                                                <th>"Source A"</th>
                                                <th>"Source B"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {r.field_mismatches.into_iter().take(100).map(|m| {
                                                view! {
                                                    <tr>
                                                        <td>{m.key_value}</td>
                                                        <td><code>{m.column_name}</code></td>
                                                        <td>{m.source1_value}</td>
                                                        <td>{m.source2_value}</td>
                                                    </tr>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                }.into_view()
                            } else {
                                view! { <p class="empty">"No field mismatches."</p> }.into_view()
                            }}
                            <p class="muted">{r.summary}</p>
                        </div>
                    }
                }).collect::<Vec<_>>().into_view()
            }}
        </div>
    }
}

// ── Multi-Value tab ───────────────────────────────────────────────────────────

#[component]
fn MvTab(result: Rc<AnalysisResult>) -> impl IntoView {
    let mvs = result.multi_value_analyses.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Multi-Value Columns"</h2>
            {if mvs.is_empty() {
                view! { <p class="empty">"No multi-value columns detected."</p> }.into_view()
            } else {
                mvs.into_iter().map(|mv| {
                    view! {
                        <div class="mv-card">
                            <h3>{mv.table_name.clone()}</h3>
                            {if mv.multi_value_columns.is_empty() {
                                view! { <p class="empty">"None found."</p> }.into_view()
                            } else {
                                view! {
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Column"</th>
                                                <th>"Delimiter"</th>
                                                <th>"Confidence"</th>
                                                <th>"Multi-value ratio"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {mv.multi_value_columns.into_iter().map(|col| {
                                                let delim = col.delimiter
                                                    .as_deref()
                                                    .unwrap_or("—")
                                                    .to_string();
                                                let conf = format!("{:.0}%", col.confidence * 100.0);
                                                let ratio = format!("{:.1}%", col.multi_value_ratio * 100.0);
                                                view! {
                                                    <tr>
                                                        <td><code>{col.column_name}</code></td>
                                                        <td><code>{delim}</code></td>
                                                        <td>{conf}</td>
                                                        <td>{ratio}</td>
                                                    </tr>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                }.into_view()
                            }}
                        </div>
                    }
                }).collect::<Vec<_>>().into_view()
            }}
        </div>
    }
}
