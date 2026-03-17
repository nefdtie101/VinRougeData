use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use vinrouge::analysis::Workflow;
use vinrouge::schema::{Relationship, RelationshipType, Table};
use crate::types::AnalysisResult;
use crate::ollama::{OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, ask_ollama_wasm, build_web_summary};

// ── Results component ─────────────────────────────────────────────────────────

#[component]
pub fn Results(result: AnalysisResult) -> impl IntoView {
    let col_count: usize = result.tables.iter().map(|t| t.columns.len()).sum();
    let tables = result.tables.clone();
    let relationships = result.relationships.clone();
    let workflows = result.workflows.clone();

    view! {
        <div class="summary-grid">
            <div class="summary-card tables">
                <div class="label">"Tables / Sheets"</div>
                <div class="value">{result.tables.len()}</div>
            </div>
            <div class="summary-card cols">
                <div class="label">"Columns"</div>
                <div class="value">{col_count}</div>
            </div>
            <div class="summary-card rels">
                <div class="label">"Relationships"</div>
                <div class="value">{result.relationships.len()}</div>
            </div>
            <div class="summary-card flows">
                <div class="label">"Workflows"</div>
                <div class="value">{result.workflows.len()}</div>
            </div>
        </div>

        <section>
            <h2>"Schema"</h2>
            {tables.into_iter().map(|t| view! { <TableCard table=t /> }).collect_view()}
        </section>

        <section>
            <h2>"Relationships"</h2>
            {if relationships.is_empty() {
                view! { <p class="empty-state">"No relationships detected."</p> }.into_any()
            } else {
                relationships.into_iter().map(|r| view! { <RelItem rel=r /> }).collect_view().into_any()
            }}
        </section>

        <section>
            <h2>"Workflows"</h2>
            {if workflows.is_empty() {
                view! { <p class="empty-state">"No workflow patterns detected."</p> }.into_any()
            } else {
                workflows.into_iter().map(|w| view! { <WorkflowCard workflow=w /> }).collect_view().into_any()
            }}
        </section>

    }
}

// ── TableCard ─────────────────────────────────────────────────────────────────

#[component]
pub fn TableCard(table: Table) -> impl IntoView {
    let open = RwSignal::new(true);
    let rows = table
        .row_count
        .map(|r| format!("{r} rows"))
        .unwrap_or_default();
    let cols = format!("{} cols", table.columns.len());
    let name = table.name.clone();
    let columns = table.columns.clone();

    view! {
        <div class="table-card">
            <div class="table-card-header" on:click=move |_| open.update(|v| *v = !*v)>
                <span class="table-name">{name}</span>
                <span class="table-meta">
                    <span>{cols}</span>
                    {(!rows.is_empty()).then(|| view! { <span>{rows}</span> })}
                    <span>{move || if open.get() { "▲" } else { "▼" }}</span>
                </span>
            </div>

            {move || open.get().then(|| view! {
                <table class="columns-table">
                    <thead>
                        <tr>
                            <th>"Column"</th><th>"Type"</th><th>"Flags"</th><th>"Samples"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {columns.iter().map(|c| {
                            let samples = c.sample_values.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
                            let type_str = format!("{:?}", c.data_type);
                            let mut flags = Vec::new();
                            if c.is_primary_key { flags.push("PK"); }
                            if c.nullable       { flags.push("null"); }
                            if c.is_foreign_key { flags.push("FK"); }
                            let col_name = c.name.clone();
                            view! {
                                <tr>
                                    <td class="col-name">{col_name}</td>
                                    <td class="col-type">{type_str}</td>
                                    <td class="col-pk">{flags.join(" ")}</td>
                                    <td class="col-samples">{samples}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            })}
        </div>
    }
}

// ── RelItem ───────────────────────────────────────────────────────────────────

#[component]
pub fn RelItem(rel: Relationship) -> impl IntoView {
    let type_label = match &rel.relationship_type {
        vinrouge::schema::RelationshipType::ForeignKey => "FK".to_string(),
        vinrouge::schema::RelationshipType::NameMatch { confidence } => {
            format!("name match {confidence}%")
        }
        vinrouge::schema::RelationshipType::ValueOverlap { overlap_percent } => {
            format!("value overlap {overlap_percent}%")
        }
        vinrouge::schema::RelationshipType::UniquePattern => "unique pattern".to_string(),
        vinrouge::schema::RelationshipType::Composite => "composite".to_string(),
    };

    view! {
        <div class="rel-item">
            <span class="rel-from">{rel.from_table}"."<strong>{rel.from_column}</strong></span>
            <span class="rel-arrow">"→"</span>
            <span class="rel-to">{rel.to_table}"."<strong>{rel.to_column}</strong></span>
            <span class="rel-type">{type_label}</span>
        </div>
    }
}

// ── WorkflowCard ──────────────────────────────────────────────────────────────

#[component]
pub fn WorkflowCard(workflow: Workflow) -> impl IntoView {
    let wtype = format!("{:?}", workflow.workflow_type);
    let conf = format!("confidence {}%", workflow.confidence);
    let desc = workflow.description.clone();
    let steps = workflow.steps.clone();

    view! {
        <div class="workflow-card">
            <div class="workflow-header">
                <span class="workflow-type">{wtype}</span>
                <span class="workflow-confidence">{conf}</span>
            </div>
            <p class="workflow-desc">{desc}</p>
            <div class="workflow-steps">
                {steps.into_iter().enumerate().map(|(i, step)| {
                    let name = step.table_name.clone();
                    view! {
                        {(i > 0).then(|| view! { <span class="step-arrow">"→"</span> })}
                        <span class="workflow-step">{name}</span>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

// ── OllamaSection ─────────────────────────────────────────────────────────────

#[component]
pub fn OllamaSection<F>(summary: F) -> impl IntoView
where
    F: Fn() -> String + 'static,
{
    let question: RwSignal<String> = RwSignal::new(String::new());
    let response: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let url: RwSignal<String> = RwSignal::new(OLLAMA_DEFAULT_URL.to_string());

    let on_submit = move |_| {
        let q = question.get();
        if q.trim().is_empty() {
            return;
        }
        let ctx = summary();
        let ollama_url = url.get();
        let ollama_model = OLLAMA_DEFAULT_MODEL.to_string();

        loading.set(true);
        error.set(None);
        response.set(None);

        spawn_local(async move {
            match ask_ollama_wasm(&ollama_url, &ollama_model, &ctx, &q).await {
                Ok(ans) => response.set(Some(ans)),
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };

    view! {
        <section class="ollama-section">
            <h2>"Ask Ollama"</h2>
            <p class="ollama-hint">
                "Ask questions about your data schema using a locally-running Ollama model. "
                "Requires Ollama to be running with "
                <code>"OLLAMA_ORIGINS=*"</code>
                " (for browser CORS)."
            </p>

            <div class="ollama-config">
                <label>
                    "Endpoint: "
                    <input
                        type="text"
                        class="ollama-input"
                        prop:value=move || url.get()
                        on:input=move |ev| url.set(event_target_value(&ev))
                        placeholder=OLLAMA_DEFAULT_URL
                    />
                </label>
            </div>

            <div class="ollama-query-row">
                <textarea
                    class="ollama-textarea"
                    rows="3"
                    placeholder="e.g. What relationships exist between these tables?"
                    prop:value=move || question.get()
                    on:input=move |ev| question.set(event_target_value(&ev))
                />
                <button
                    class="ollama-btn"
                    on:click=on_submit
                    disabled=move || loading.get()
                >
                    {move || if loading.get() { "Thinking…" } else { "Ask" }}
                </button>
            </div>

            {move || error.get().map(|e| view! {
                <div class="ollama-error">"Error: " {e}</div>
            })}

            {move || response.get().map(|r| view! {
                <div class="ollama-response">
                    <h3>"Response"</h3>
                    <pre class="ollama-response-text">{r}</pre>
                </div>
            })}
        </section>
    }
}
