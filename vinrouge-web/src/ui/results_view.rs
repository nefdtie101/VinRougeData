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
                    view! {
                        <div class="profile-card">
                            <h3>{p.source_name.clone()}</h3>
                            <p>"Rows: " {p.row_count} "  ·  Columns: " {p.column_count}</p>
                            <table class="data-table">
                                <thead>
                                    <tr>
                                        <th>"Column"</th>
                                        <th>"Pattern"</th>
                                        <th>"Completeness"</th>
                                        <th>"Cardinality"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {p.column_profiles.into_iter().map(|cp| {
                                        let pct = format!("{:.1}%", cp.completeness * 100.0);
                                        let card = format!("{:.1}%", cp.cardinality * 100.0);
                                        view! {
                                            <tr>
                                                <td>{cp.column_name}</td>
                                                <td><code>{format!("{:?}", cp.pattern_type)}</code></td>
                                                <td>{pct}</td>
                                                <td>{card}</td>
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
                                <th>"Confidence"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rels.into_iter().map(|r| {
                                let conf = format!("{:.0}%", r.confidence * 100.0);
                                view! {
                                    <tr>
                                        <td>{r.from_table}</td>
                                        <td><code>{r.from_column}</code></td>
                                        <td>"→"</td>
                                        <td>{r.to_table}</td>
                                        <td><code>{r.to_column}</code></td>
                                        <td>{conf}</td>
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
                    view! {
                        <div class="recon-card">
                            <h3>{r.column_name.clone()}</h3>
                            <p>
                                "Only in A: " {r.only_in_source_a.len()}
                                "  ·  Only in B: " {r.only_in_source_b.len()}
                                "  ·  Mismatches: " {r.mismatches.len()}
                            </p>
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
                            <h3>{mv.source_name.clone()}</h3>
                            <ul>
                                {mv.multi_value_columns.into_iter().map(|col| {
                                    view! { <li><code>{col.column_name}</code> " — delimiter: " {col.delimiter}</li> }
                                }).collect::<Vec<_>>()}
                            </ul>
                        </div>
                    }
                }).collect::<Vec<_>>().into_view()
            }}
        </div>
    }
}
