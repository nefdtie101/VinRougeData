use js_sys::Uint8Array;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use vinrouge::analysis::{RelationshipDetector, Workflow, WorkflowDetector};
use vinrouge::schema::{Relationship, Table};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};

// ── Ollama constants ──────────────────────────────────────────────────────────

const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434";
const OLLAMA_DEFAULT_MODEL: &str = "mistral";

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

// ── Domain ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Deserialize)]
struct AnalysisResult {
    tables: Vec<Table>,
    relationships: Vec<Relationship>,
    workflows: Vec<Workflow>,
}

// ── Tauri detection & IPC ─────────────────────────────────────────────────────

/// Returns true when the page is running inside a Tauri WebView.
fn is_tauri() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::has(&w, &JsValue::from_str("__TAURI__")).ok())
        .unwrap_or(false)
}

/// Call `pick_and_analyze` Tauri command (opens OS file dialog, runs analysis
/// in native Rust, returns JSON). Returns `None` if the user cancelled.
async fn tauri_pick_and_analyze() -> Result<Option<AnalysisResult>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function =
        js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
            .map_err(|_| "no invoke")?
            .dyn_into()
            .map_err(|_| "invoke not a function")?;

    let promise: js_sys::Promise = invoke
        .call1(&JsValue::UNDEFINED, &JsValue::from_str("pick_and_analyze"))
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    if val.is_null() || val.is_undefined() {
        return Ok(None); // user cancelled
    }

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<AnalysisResult>(&json)
        .map(Some)
        .map_err(|e| format!("deserialize: {e}"))
}

// ── Browser file analysis (WASM) ──────────────────────────────────────────────

async fn analyze_bytes(bytes: Vec<u8>, name: &str) -> Result<AnalysisResult, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();

    let tables: Vec<Table> = if ext == "csv" {
        CsvSource::from_bytes(bytes, name.to_string())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else if ext == "xlsx" || ext == "xls" {
        ExcelSource::from_bytes(bytes, name.to_string())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else {
        return Err(format!("Unsupported file type: .{ext}"));
    };

    let relationships = RelationshipDetector::new(tables.clone()).detect_relationships();
    let workflows =
        WorkflowDetector::new(tables.clone(), relationships.clone()).detect_workflows();

    Ok(AnalysisResult { tables, relationships, workflows })
}

async fn read_file_bytes(file: &web_sys::File) -> Result<Vec<u8>, JsValue> {
    let buf = JsFuture::from(file.array_buffer()).await?;
    Ok(Uint8Array::new(&buf).to_vec())
}

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> impl IntoView {
    let result: RwSignal<Option<AnalysisResult>> = RwSignal::new(None);
    let status: RwSignal<String> = RwSignal::new(String::new());
    let loading: RwSignal<bool> = RwSignal::new(false);
    let active_tab: RwSignal<&'static str> = RwSignal::new("chat");
    let tauri = is_tauri();

    // ── Tauri: native "Open File" button ──────────────────────────────────────

    let on_open_native = move |_| {
        loading.set(true);
        status.set("Opening file…".into());
        result.set(None);
        spawn_local(async move {
            match tauri_pick_and_analyze().await {
                Ok(Some(r)) => {
                    result.set(Some(r));
                    status.set(String::new());
                }
                Ok(None) => {
                    // cancelled — clear the spinner but don't show an error
                    status.set(String::new());
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
            loading.set(false);
        });
    };

    // ── Browser: file-input + drag-and-drop ───────────────────────────────────

    let process_file = move |file: web_sys::File| {
        let name = file.name();
        loading.set(true);
        status.set(format!("Analyzing {name}…"));
        result.set(None);
        spawn_local(async move {
            match read_file_bytes(&file).await {
                Ok(bytes) => match analyze_bytes(bytes, &name).await {
                    Ok(r) => {
                        result.set(Some(r));
                        status.set(String::new());
                    }
                    Err(e) => status.set(format!("Error: {e}")),
                },
                Err(e) => status.set(format!("Could not read file: {e:?}")),
            }
            loading.set(false);
        });
    };

    let on_file_input = move |ev: web_sys::Event| {
        let input: web_sys::HtmlInputElement =
            wasm_bindgen::JsCast::unchecked_into(ev.target().unwrap());
        if let Some(files) = input.files() {
            if let Some(file) = files.get(0) {
                process_file(file);
            }
        }
    };

    let on_drag_over = move |ev: web_sys::DragEvent| ev.prevent_default();

    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            if let Some(files) = dt.files() {
                if let Some(file) = files.get(0) {
                    process_file(file);
                }
            }
        }
    };

    // ── Subtitle changes depending on context ─────────────────────────────────

    let subtitle = if tauri {
        "Standalone desktop app — click Open File to get started"
    } else {
        "Data structure analysis — runs entirely in your browser"
    };

    view! {
        <header>
            <h1>"VinRouge"</h1>
            <nav class="top-nav">
                <button
                    class=move || if active_tab.get() == "chat" { "nav-btn active" } else { "nav-btn" }
                    on:click=move |_| active_tab.set("chat")
                >"Chat"</button>
                <button
                    class=move || if active_tab.get() == "data" { "nav-btn active" } else { "nav-btn" }
                    on:click=move |_| active_tab.set("data")
                >"File Upload"</button>
            </nav>
            <p>{subtitle}</p>
        </header>

        <main>
            // ── Chat tab ──────────────────────────────────────────────────────
            {move || (active_tab.get() == "chat").then(|| view! {
                <OllamaSection summary=move || result.get().map(|r| build_web_summary(&r)).unwrap_or_default() />
            })}

            // ── File Upload tab ───────────────────────────────────────────────
            {move || (active_tab.get() == "data").then(|| view! {
                <div>
                    {if tauri {
                        view! {
                            <div class="upload-zone" on:click=on_open_native>
                                <div class="upload-icon">"📂"</div>
                                <div>"Click to open a CSV or Excel file"</div>
                                <div class="hint">"Supported: .csv  .xlsx  .xls"</div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div
                                class="upload-zone"
                                on:dragover=on_drag_over
                                on:drop=on_drop
                            >
                                <label for="file-input">
                                    <div class="upload-icon">"📂"</div>
                                    <div>"Drop a CSV or Excel file here, or click to browse"</div>
                                    <div class="hint">"Supported: .csv  .xlsx  .xls"</div>
                                    <input
                                        id="file-input"
                                        type="file"
                                        accept=".csv,.xlsx,.xls"
                                        on:change=on_file_input
                                    />
                                </label>
                            </div>
                        }.into_any()
                    }}

                    {move || {
                        let s = status.get();
                        if loading.get() {
                            Some(view! { <div class="status-bar loading">"⏳  " {s}</div> })
                        } else if !s.is_empty() {
                            Some(view! { <div class="status-bar error">"⚠  " {s}</div> })
                        } else {
                            None
                        }
                    }}

                    {move || result.get().map(|r| view! { <Results result=r /> })}
                </div>
            })}
        </main>
    }
}

// ── Results component ─────────────────────────────────────────────────────────

#[component]
fn Results(result: AnalysisResult) -> impl IntoView {
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
fn TableCard(table: Table) -> impl IntoView {
    let open = RwSignal::new(true);
    let rows = table.row_count.map(|r| format!("{r} rows")).unwrap_or_default();
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
fn RelItem(rel: Relationship) -> impl IntoView {
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
fn WorkflowCard(workflow: Workflow) -> impl IntoView {
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
fn OllamaSection<F>(summary: F) -> impl IntoView
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

// ── Ollama HTTP helper (WASM) ─────────────────────────────────────────────────

async fn ask_ollama_wasm(
    base_url: &str,
    model: &str,
    context: &str,
    question: &str,
) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    let prompt = if context.trim().is_empty() {
        format!("You are a data analyst assistant. Answer the following question concisely:\n\n{question}")
    } else {
        format!(
            "You are a data analyst assistant. The user analysed a dataset with the following \
             schema and findings:\n\n{context}\n\nBased on this, answer concisely:\n\n{question}"
        )
    };

    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let resp = Request::post(&url)
        .json(&body)
        .map_err(|e| format!("Failed to build request: {e}"))?
        .send()
        .await
        .map_err(|e| {
            format!(
                "Could not reach Ollama at {url}. \
                 Is it running? Did you set OLLAMA_ORIGINS=*? Error: {e}"
            )
        })?;

    if !resp.ok() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama returned HTTP {status}: {text}"));
    }

    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected response shape: {val}"))
}

// ── Analysis summary for Ollama context ───────────────────────────────────────

fn build_web_summary(result: &AnalysisResult) -> String {
    let mut s = String::new();

    s.push_str(&format!("Tables ({}):\n", result.tables.len()));
    for t in &result.tables {
        s.push_str(&format!(
            "  - {} ({} columns, ~{} rows)\n",
            t.name,
            t.columns.len(),
            t.row_count.unwrap_or(0)
        ));
        for c in &t.columns {
            s.push_str(&format!("      {}: {:?}\n", c.name, c.data_type));
        }
    }

    if !result.relationships.is_empty() {
        s.push_str(&format!("\nRelationships ({}):\n", result.relationships.len()));
        for r in &result.relationships {
            s.push_str(&format!(
                "  - {}.{} -> {}.{}\n",
                r.from_table, r.from_column, r.to_table, r.to_column
            ));
        }
    }

    if !result.workflows.is_empty() {
        s.push_str(&format!("\nWorkflows ({}):\n", result.workflows.len()));
        for w in &result.workflows {
            s.push_str(&format!("  - {:?}: {}\n", w.workflow_type, w.description));
        }
    }

    s
}
