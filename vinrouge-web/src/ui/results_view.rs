use leptos::prelude::*;
use std::sync::Arc;
use vinrouge::analysis::{
    DataProfile, DimensionType, FieldMismatch, GroupingAnalysis, MultiValueAnalysis,
    ReconciliationResult,
};
use vinrouge::export::AnalysisResult;
use vinrouge::schema::Table;

#[component]
pub fn ResultsView(result: Arc<AnalysisResult>) -> impl IntoView {
    let (active_tab, set_tab) = signal("schema");

    let result_schema = result.clone();
    let result_profile = result.clone();
    let result_rel = result.clone();
    let result_recon = result.clone();
    let result_mv = result.clone();
    let result_grouping = result.clone();

    view! {
        <div class="results-panel">
            <nav class="tab-bar">
                <TabButton label="Schema" id="schema" active=active_tab set_tab=set_tab />
                <TabButton label="Data Profile" id="profile" active=active_tab set_tab=set_tab />
                <TabButton label="Grouping" id="grouping" active=active_tab set_tab=set_tab />
                <TabButton label="Relationships" id="relationships" active=active_tab set_tab=set_tab />
                <TabButton label="Reconciliation" id="reconciliation" active=active_tab set_tab=set_tab />
                <TabButton label="Multi-Value" id="multivalue" active=active_tab set_tab=set_tab />
            </nav>

            <div class="tab-content">
                {move || match active_tab.get() {
                    "schema" => view! { <SchemaTab result=result_schema.clone() /> }.into_any(),
                    "profile" => view! { <ProfileTab result=result_profile.clone() /> }.into_any(),
                    "grouping" => view! { <GroupingTab result=result_grouping.clone() /> }.into_any(),
                    "relationships" => view! { <RelTab result=result_rel.clone() /> }.into_any(),
                    "reconciliation" => view! { <ReconTab result=result_recon.clone() /> }.into_any(),
                    "multivalue" => view! { <MvTab result=result_mv.clone() /> }.into_any(),
                    _ => view! { <div /> }.into_any(),
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

// ── Sort header ─────────────────────────────────────────────────────────────

#[component]
fn SortTh(
    label: &'static str,
    col_key: &'static str,
    sort: ReadSignal<(&'static str, bool)>,
    set_sort: WriteSignal<(&'static str, bool)>,
) -> impl IntoView {
    view! {
        <th
            class="sortable-th"
            on:click=move |_| {
                let (cur_col, cur_asc) = sort.get();
                if cur_col == col_key {
                    set_sort.set((col_key, !cur_asc));
                } else {
                    set_sort.set((col_key, true));
                }
            }
        >
            {label}
            <span class="sort-indicator">
                {move || {
                    let (cur_col, cur_asc) = sort.get();
                    if cur_col == col_key {
                        if cur_asc { "▲" } else { "▼" }
                    } else {
                        "⇅"
                    }
                }}
            </span>
        </th>
    }
}

// ── Schema ─────────────────────────────────────────────────────────────────

#[component]
fn SchemaTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let tables = result.tables.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Schema — " {tables.len()} " tables"</h2>
            {tables.into_iter().map(|table| {
                view! { <SchemaTableCard table=table /> }
            }).collect_view()}
        </div>
    }
}

#[component]
fn SchemaTableCard(table: Table) -> impl IntoView {
    let (sort, set_sort) = signal(("name", true));
    let cols_orig = table.columns.clone();
    let col_count = cols_orig.len();

    view! {
        <details class="table-card" open=true>
            <summary class="table-card__header">
                <strong>{table.name.clone()}</strong>
                <span class="badge">{col_count} " columns"</span>
                {table.row_count.map(|r| view! {
                    <span class="badge badge--grey">{r} " rows"</span>
                })}
            </summary>
            <table class="data-table">
                <thead>
                    <tr>
                        <SortTh label="Column" col_key="name" sort=sort set_sort=set_sort />
                        <SortTh label="Type" col_key="type" sort=sort set_sort=set_sort />
                        <SortTh label="Nulls" col_key="nulls" sort=sort set_sort=set_sort />
                        <SortTh label="Unique" col_key="unique" sort=sort set_sort=set_sort />
                        <th>"Sample values"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        let mut cols = cols_orig.clone();
                        let (col_key, asc) = sort.get();
                        match col_key {
                            "type" => cols.sort_by(|a, b| {
                                format!("{:?}", a.data_type)
                                    .cmp(&format!("{:?}", b.data_type))
                            }),
                            "nulls" => cols.sort_by_key(|c| c.null_count.unwrap_or(0)),
                            "unique" => cols.sort_by_key(|c| c.unique_count.unwrap_or(0)),
                            _ => cols.sort_by(|a, b| {
                                a.name.to_lowercase().cmp(&b.name.to_lowercase())
                            }),
                        }
                        if !asc {
                            cols.reverse();
                        }
                        cols.into_iter()
                            .map(|col| {
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
                            })
                            .collect_view()
                    }}
                </tbody>
            </table>
        </details>
    }
}

// ── Data Profile ───────────────────────────────────────────────────────────

#[component]
fn ProfileTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let profiles = result.data_profiles.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Data Profiles"</h2>
            {if profiles.is_empty() {
                view! { <p class="empty">"No data profiles available."</p> }.into_any()
            } else {
                profiles
                    .into_iter()
                    .map(|p| view! { <ProfileCard profile=p /> })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn ProfileCard(profile: DataProfile) -> impl IntoView {
    let (sort, set_sort) = signal(("column", true));
    let col_count = profile.column_profiles.len();
    let cols_orig = profile.column_profiles.clone();

    view! {
        <div class="profile-card">
            <h3>{profile.table_name.clone()}</h3>
            <p>"Columns profiled: " {col_count}</p>
            <table class="data-table">
                <thead>
                    <tr>
                        <SortTh label="Column" col_key="column" sort=sort set_sort=set_sort />
                        <SortTh label="Total" col_key="total" sort=sort set_sort=set_sort />
                        <SortTh label="Unique" col_key="unique" sort=sort set_sort=set_sort />
                        <SortTh label="Nulls" col_key="nulls" sort=sort set_sort=set_sort />
                        <SortTh label="Distinct %" col_key="distinct" sort=sort set_sort=set_sort />
                        <th>"Patterns"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        let mut cols = cols_orig.clone();
                        let (col_key, asc) = sort.get();
                        match col_key {
                            "total" => cols.sort_by_key(|c| c.total_values),
                            "unique" => cols.sort_by_key(|c| c.unique_values),
                            "nulls" => cols.sort_by_key(|c| c.null_count),
                            "distinct" => cols.sort_by(|a, b| {
                                a.distinct_ratio
                                    .partial_cmp(&b.distinct_ratio)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            }),
                            _ => cols.sort_by(|a, b| {
                                a.column_name.to_lowercase().cmp(&b.column_name.to_lowercase())
                            }),
                        }
                        if !asc {
                            cols.reverse();
                        }
                        cols.into_iter()
                            .map(|cp| {
                                let ratio = format!("{:.1}%", cp.distinct_ratio * 100.0);
                                let patterns = cp
                                    .data_patterns
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
                            })
                            .collect_view()
                    }}
                </tbody>
            </table>
        </div>
    }
}

// ── Relationships ──────────────────────────────────────────────────────────

#[component]
fn RelTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let rels_orig = result.relationships.clone();
    let rel_count = rels_orig.len();
    let (sort, set_sort) = signal(("from_table", true));

    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Relationships — " {rel_count}</h2>
            {if rels_orig.is_empty() {
                view! { <p class="empty">"No relationships detected."</p> }.into_any()
            } else {
                view! {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <SortTh label="From table" col_key="from_table" sort=sort set_sort=set_sort />
                                <SortTh label="Column" col_key="from_col" sort=sort set_sort=set_sort />
                                <th>"→"</th>
                                <SortTh label="To table" col_key="to_table" sort=sort set_sort=set_sort />
                                <SortTh label="Column" col_key="to_col" sort=sort set_sort=set_sort />
                                <SortTh label="Type" col_key="rel_type" sort=sort set_sort=set_sort />
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                let mut rels = rels_orig.clone();
                                let (col_key, asc) = sort.get();
                                match col_key {
                                    "from_col" => rels.sort_by(|a, b| {
                                        a.from_column.to_lowercase().cmp(&b.from_column.to_lowercase())
                                    }),
                                    "to_table" => rels.sort_by(|a, b| {
                                        a.to_table.to_lowercase().cmp(&b.to_table.to_lowercase())
                                    }),
                                    "to_col" => rels.sort_by(|a, b| {
                                        a.to_column.to_lowercase().cmp(&b.to_column.to_lowercase())
                                    }),
                                    "rel_type" => rels.sort_by(|a, b| {
                                        format!("{:?}", a.relationship_type)
                                            .cmp(&format!("{:?}", b.relationship_type))
                                    }),
                                    _ => rels.sort_by(|a, b| {
                                        a.from_table.to_lowercase().cmp(&b.from_table.to_lowercase())
                                    }),
                                }
                                if !asc {
                                    rels.reverse();
                                }
                                rels.into_iter()
                                    .map(|r| {
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
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                }
                .into_any()
            }}
        </div>
    }
}

// ── Reconciliation ─────────────────────────────────────────────────────────

#[component]
fn ReconTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let recons = result.reconciliation_results.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Reconciliation"</h2>
            {if recons.is_empty() {
                view! {
                    <p class="empty">"Upload at least two files to see reconciliation results."</p>
                }
                .into_any()
            } else {
                recons
                    .into_iter()
                    .map(|r| view! { <ReconCard recon=r /> })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn ReconCard(recon: ReconciliationResult) -> impl IntoView {
    let (sort, set_sort) = signal(("key", true));
    let pct = format!("{:.1}%", recon.match_percentage);
    let mismatches_orig = recon.field_mismatches.clone();
    let has_mismatches = !mismatches_orig.is_empty();

    view! {
        <div class="recon-card">
            <h3>{recon.source1_name.clone()} " vs " {recon.source2_name.clone()}</h3>
            <p>
                "Match: " {pct}
                "  ·  A: " {recon.total_source1}
                "  ·  B: " {recon.total_source2}
                "  ·  Matched: " {recon.matches}
                "  ·  Only in A: " {recon.only_in_source1}
                "  ·  Only in B: " {recon.only_in_source2}
            </p>
            {if has_mismatches {
                view! {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <SortTh label="Key" col_key="key" sort=sort set_sort=set_sort />
                                <SortTh label="Column" col_key="column" sort=sort set_sort=set_sort />
                                <SortTh label="Source A" col_key="src_a" sort=sort set_sort=set_sort />
                                <SortTh label="Source B" col_key="src_b" sort=sort set_sort=set_sort />
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                let mut items: Vec<FieldMismatch> =
                                    mismatches_orig.iter().take(100).cloned().collect();
                                let (col_key, asc) = sort.get();
                                match col_key {
                                    "column" => items.sort_by(|a, b| {
                                        a.column_name.to_lowercase().cmp(&b.column_name.to_lowercase())
                                    }),
                                    "src_a" => items.sort_by(|a, b| {
                                        a.source1_value.to_lowercase().cmp(&b.source1_value.to_lowercase())
                                    }),
                                    "src_b" => items.sort_by(|a, b| {
                                        a.source2_value.to_lowercase().cmp(&b.source2_value.to_lowercase())
                                    }),
                                    _ => items.sort_by(|a, b| {
                                        a.key_value.to_lowercase().cmp(&b.key_value.to_lowercase())
                                    }),
                                }
                                if !asc {
                                    items.reverse();
                                }
                                items
                                    .into_iter()
                                    .map(|m| {
                                        view! {
                                            <tr>
                                                <td>{m.key_value}</td>
                                                <td><code>{m.column_name}</code></td>
                                                <td>{m.source1_value}</td>
                                                <td>{m.source2_value}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                }
                .into_any()
            } else {
                view! { <p class="empty">"No field mismatches."</p> }.into_any()
            }}
            <p class="muted">{recon.summary}</p>
        </div>
    }
}

// ── Grouping ───────────────────────────────────────────────────────────────

#[component]
fn GroupingTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let groupings = result.grouping_analyses.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Grouping Analysis"</h2>
            {if groupings.is_empty() {
                view! { <p class="empty">"No grouping analyses available."</p> }.into_any()
            } else {
                groupings
                    .into_iter()
                    .map(|g| view! { <GroupingCard grouping=g /> })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn GroupingCard(grouping: GroupingAnalysis) -> impl IntoView {
    let (sort, set_sort) = signal(("column", true));
    let dims_orig = grouping.grouping_dimensions.clone();
    let hierarchies = grouping.hierarchies.clone();
    let suggestions = grouping.suggested_analyses.clone();
    let has_dims = !dims_orig.is_empty();

    view! {
        <div class="mv-card">
            <h3>{grouping.table_name.clone()}</h3>

            {if has_dims {
                view! {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <SortTh label="Column" col_key="column" sort=sort set_sort=set_sort />
                                <SortTh label="Type" col_key="type" sort=sort set_sort=set_sort />
                                <SortTh label="Groups" col_key="groups" sort=sort set_sort=set_sort />
                                <SortTh label="Min" col_key="min" sort=sort set_sort=set_sort />
                                <SortTh label="Max" col_key="max" sort=sort set_sort=set_sort />
                                <SortTh label="Avg" col_key="avg" sort=sort set_sort=set_sort />
                                <th>"Top groups"</th>
                                <th>"Insights"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                let mut dims = dims_orig.clone();
                                let (col_key, asc) = sort.get();
                                match col_key {
                                    "type" => dims.sort_by(|a, b| {
                                        format!("{:?}", a.dimension_type)
                                            .cmp(&format!("{:?}", b.dimension_type))
                                    }),
                                    "groups" => dims.sort_by_key(|d| d.group_count),
                                    "min" => dims.sort_by_key(|d| d.records_per_group.min),
                                    "max" => dims.sort_by_key(|d| d.records_per_group.max),
                                    "avg" => dims.sort_by(|a, b| {
                                        a.records_per_group
                                            .avg
                                            .partial_cmp(&b.records_per_group.avg)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    }),
                                    _ => dims.sort_by(|a, b| {
                                        a.column_name.to_lowercase().cmp(&b.column_name.to_lowercase())
                                    }),
                                }
                                if !asc {
                                    dims.reverse();
                                }
                                dims.into_iter()
                                    .map(|d| {
                                        let dtype = match d.dimension_type {
                                            DimensionType::Temporal => "Temporal",
                                            DimensionType::Categorical => "Categorical",
                                            DimensionType::Geographic => "Geographic",
                                            DimensionType::Hierarchical => "Hierarchical",
                                            DimensionType::Identifier => "Identifier",
                                            DimensionType::Numeric => "Numeric",
                                        };
                                        let avg = format!("{:.1}", d.records_per_group.avg);
                                        let top_groups = d
                                            .example_groups
                                            .iter()
                                            .map(|eg| format!("{} ({})", eg.group_value, eg.record_count))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let insights = d.insights.join("; ");
                                        view! {
                                            <tr>
                                                <td><code>{d.column_name}</code></td>
                                                <td>{dtype}</td>
                                                <td>{d.group_count}</td>
                                                <td>{d.records_per_group.min}</td>
                                                <td>{d.records_per_group.max}</td>
                                                <td>{avg}</td>
                                                <td class="samples">{top_groups}</td>
                                                <td class="samples">{insights}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                }
                .into_any()
            } else {
                view! { <p class="empty">"No grouping dimensions detected."</p> }.into_any()
            }}

            {if !hierarchies.is_empty() {
                let hier_list = hierarchies
                    .iter()
                    .map(|h| format!("{} ({})", h.levels.join(" → "), h.description))
                    .collect::<Vec<_>>()
                    .join(" | ");
                view! {
                    <p class="muted"><strong>"Hierarchies: "</strong>{hier_list}</p>
                }.into_any()
            } else {
                view! { <span /> }.into_any()
            }}

            {if !suggestions.is_empty() {
                view! {
                    <details>
                        <summary class="muted">"Suggested analyses"</summary>
                        <ul class="suggestions-list">
                            {suggestions.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                        </ul>
                    </details>
                }.into_any()
            } else {
                view! { <span /> }.into_any()
            }}
        </div>
    }
}

// ── Multi-Value ────────────────────────────────────────────────────────────

#[component]
fn MvTab(result: Arc<AnalysisResult>) -> impl IntoView {
    let mvs = result.multi_value_analyses.clone();
    view! {
        <div class="tab-pane">
            <h2 class="section-title">"Multi-Value Columns"</h2>
            {if mvs.is_empty() {
                view! { <p class="empty">"No multi-value columns detected."</p> }.into_any()
            } else {
                mvs.into_iter()
                    .map(|mv| view! { <MvCard mv=mv /> })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn MvCard(mv: MultiValueAnalysis) -> impl IntoView {
    let (sort, set_sort) = signal(("column", true));
    let cols_orig = mv.multi_value_columns.clone();
    let has_cols = !cols_orig.is_empty();

    view! {
        <div class="mv-card">
            <h3>{mv.table_name.clone()}</h3>
            {if has_cols {
                view! {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <SortTh label="Column" col_key="column" sort=sort set_sort=set_sort />
                                <SortTh label="Delimiter" col_key="delimiter" sort=sort set_sort=set_sort />
                                <SortTh label="Confidence" col_key="confidence" sort=sort set_sort=set_sort />
                                <SortTh label="MV ratio" col_key="ratio" sort=sort set_sort=set_sort />
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                let mut cols = cols_orig.clone();
                                let (col_key, asc) = sort.get();
                                match col_key {
                                    "delimiter" => cols.sort_by(|a, b| {
                                        a.delimiter
                                            .as_deref()
                                            .unwrap_or("")
                                            .cmp(b.delimiter.as_deref().unwrap_or(""))
                                    }),
                                    "confidence" => cols.sort_by(|a, b| {
                                        a.confidence
                                            .partial_cmp(&b.confidence)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    }),
                                    "ratio" => cols.sort_by(|a, b| {
                                        a.multi_value_ratio
                                            .partial_cmp(&b.multi_value_ratio)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    }),
                                    _ => cols.sort_by(|a, b| {
                                        a.column_name.to_lowercase().cmp(&b.column_name.to_lowercase())
                                    }),
                                }
                                if !asc {
                                    cols.reverse();
                                }
                                cols.into_iter()
                                    .map(|col| {
                                        let delim = col.delimiter.as_deref().unwrap_or("—").to_string();
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
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                }
                .into_any()
            } else {
                view! { <p class="empty">"None found."</p> }.into_any()
            }}
        </div>
    }
}
