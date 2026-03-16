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

#[derive(Clone, Debug, serde::Deserialize)]
struct Project {
    id: String,
    name: String,
    path: String,
    created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ProjectFile {
    id: String,
    name: String,
    path: String,
    #[serde(rename = "type")]
    file_type: String,
    uploaded_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct AiMessage {
    id: String,
    role: String,
    content: String,
    created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Control {
    id: String,
    process_id: String,
    control_ref: String,
    control_objective: String,
    control_description: String,
    test_procedure: String,
    risk_level: String,
    sort_order: i64,
    created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct AuditProcessWithControls {
    id: String,
    sop_file_id: String,
    process_name: String,
    description: String,
    sort_order: i64,
    created_at: String,
    controls: Vec<Control>,
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
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
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

// ── Generic Tauri IPC helpers ─────────────────────────────────────────────────

async fn tauri_invoke<T: for<'de> serde::Deserialize<'de>>(cmd: &str) -> Result<T, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let promise: js_sys::Promise = invoke
        .call1(&JsValue::UNDEFINED, &JsValue::from_str(cmd))
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<T>(&json).map_err(|e| format!("deserialize: {e}"))
}

async fn tauri_invoke_args<T: for<'de> serde::Deserialize<'de>>(
    cmd: &str,
    args: serde_json::Value,
) -> Result<T, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let js_args = js_sys::JSON::parse(
        &serde_json::to_string(&args).map_err(|e| format!("args serialize: {e}"))?,
    )
    .map_err(|e| format!("JSON.parse: {e:?}"))?;

    let promise: js_sys::Promise = invoke
        .call2(&JsValue::UNDEFINED, &JsValue::from_str(cmd), &js_args)
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<T>(&json).map_err(|e| format!("deserialize: {e}"))
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
    let workflows = WorkflowDetector::new(tables.clone(), relationships.clone()).detect_workflows();

    Ok(AnalysisResult {
        tables,
        relationships,
        workflows,
    })
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
                <button
                    class=move || if active_tab.get() == "projects" { "nav-btn active" } else { "nav-btn" }
                    on:click=move |_| active_tab.set("projects")
                >"Projects"</button>
            </nav>
            <p>{subtitle}</p>
        </header>

        <main class=move || if active_tab.get() == "projects" { "projects-active" } else { "" }>
            // ── Chat tab ──────────────────────────────────────────────────────
            {move || (active_tab.get() == "chat").then(|| view! {
                <OllamaSection summary=move || result.get().map(|r| build_web_summary(&r)).unwrap_or_default() />
            })}

            // ── Projects tab ──────────────────────────────────────────────────
            {move || (active_tab.get() == "projects").then(|| view! {
                <ProjectsView />
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

// ── Projects enums ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum RightPanel { Empty, CreateWizard, ActiveProject, CreateSuccess }

// ── ProjectsView ──────────────────────────────────────────────────────────────

#[component]
fn ProjectsView() -> impl IntoView {
    // ── Core state ────────────────────────────────────────────────────────────
    let projects: RwSignal<Vec<Project>>          = RwSignal::new(vec![]);
    let active_project: RwSignal<Option<Project>> = RwSignal::new(None);
    let project_files: RwSignal<Vec<ProjectFile>> = RwSignal::new(vec![]);
    let ai_messages: RwSignal<Vec<AiMessage>>     = RwSignal::new(vec![]);
    let right_panel: RwSignal<RightPanel>         = RwSignal::new(RightPanel::Empty);
    let status: RwSignal<String>                  = RwSignal::new(String::new());
    let last_created: RwSignal<Option<Project>>   = RwSignal::new(None);
    let search: RwSignal<String>                  = RwSignal::new(String::new());

    // ── Wizard state (3 steps) ────────────────────────────────────────────────
    let wiz_step: RwSignal<u8>                    = RwSignal::new(1);
    // Step 1
    let wiz_name: RwSignal<String>                = RwSignal::new(String::new());
    let wiz_client: RwSignal<String>              = RwSignal::new(String::new());
    let wiz_ref_: RwSignal<String>                = RwSignal::new(String::new());
    let wiz_start: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_end: RwSignal<String>                 = RwSignal::new(String::new());
    let wiz_due: RwSignal<String>                 = RwSignal::new(String::new());
    let wiz_type: RwSignal<String>                = RwSignal::new("Compliance".to_string());
    let wiz_notes: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_save_dir: RwSignal<Option<String>>    = RwSignal::new(None);
    // Step 2
    let wiz_standards: RwSignal<Vec<String>>      = RwSignal::new(vec![]);
    let wiz_scope: RwSignal<String>               = RwSignal::new(String::new());
    let wiz_materiality: RwSignal<String>         = RwSignal::new(String::new());
    let wiz_risk_fw: RwSignal<String>             = RwSignal::new("High / Medium / Low".to_string());

    // ── Audit plan state ──────────────────────────────────────────────────────
    let audit_plan: RwSignal<Vec<AuditProcessWithControls>> = RwSignal::new(vec![]);
    let sop_analyzing: RwSignal<Option<String>>             = RwSignal::new(None); // file_id

    // ── Sidebar resize ────────────────────────────────────────────────────────
    let sidebar_width: RwSignal<f64> = RwSignal::new(260.0);
    let is_dragging:   RwSignal<bool> = RwSignal::new(false);

    // ── Chat state ────────────────────────────────────────────────────────────
    let chat_input: RwSignal<String>  = RwSignal::new(String::new());
    let chat_loading: RwSignal<bool>  = RwSignal::new(false);

    // ── Load projects on mount ────────────────────────────────────────────────
    spawn_local(async move {
        match tauri_invoke::<Vec<Project>>("list_projects").await {
            Ok(list) => projects.set(list),
            Err(e)   => status.set(format!("Failed to load projects: {e}")),
        }
    });

    let refresh_project_data = move || {
        spawn_local(async move {
            if let Ok(f) = tauri_invoke::<Vec<ProjectFile>>("list_project_files").await { project_files.set(f); }
            if let Ok(m) = tauri_invoke::<Vec<AiMessage>>("list_ai_messages").await      { ai_messages.set(m); }
            if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await { audit_plan.set(p); }
        });
    };

    // ── Reset wizard and open it ──────────────────────────────────────────────
    let open_wiz = move || {
        wiz_step.set(1);
        wiz_name.set(String::new());
        wiz_client.set(String::new());
        wiz_ref_.set(String::new());
        wiz_start.set(String::new());
        wiz_end.set(String::new());
        wiz_due.set(String::new());
        wiz_type.set("Compliance".to_string());
        wiz_notes.set(String::new());
        wiz_save_dir.set(None);
        wiz_standards.set(vec![]);
        wiz_scope.set(String::new());
        wiz_materiality.set(String::new());
        wiz_risk_fw.set("High / Medium / Low".to_string());
        right_panel.set(RightPanel::CreateWizard);
    };

    // ── Open a project ────────────────────────────────────────────────────────
    let open_project = move |path: String| {
        spawn_local(async move {
            match tauri_invoke_args::<Project>(
                "open_project",
                serde_json::json!({ "path": path }),
            ).await {
                Ok(p) => {
                    active_project.set(Some(p));
                    right_panel.set(RightPanel::ActiveProject);
                    refresh_project_data();
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Pick save folder ──────────────────────────────────────────────────────
    let on_pick_folder = move |_| {
        spawn_local(async move {
            match tauri_invoke::<Option<String>>("pick_project_folder").await {
                Ok(Some(p)) => wiz_save_dir.set(Some(p)),
                Ok(None)    => {}
                Err(e)      => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Create project (step 3 confirm) ──────────────────────────────────────
    let on_create = move |_| {
        let name = wiz_name.get();
        if name.trim().is_empty() { return; }
        let args = serde_json::json!({
            "name":           name,
            "saveDir":        wiz_save_dir.get(),
            "client":         wiz_client.get(),
            "engagementRef":  wiz_ref_.get(),
            "periodStart":    wiz_start.get(),
            "periodEnd":      wiz_end.get(),
            "reportDue":      wiz_due.get(),
            "auditType":      wiz_type.get(),
            "notes":          wiz_notes.get(),
            "standards":      wiz_standards.get(),
            "scope":          wiz_scope.get(),
            "materiality":    wiz_materiality.get(),
            "riskFramework":  wiz_risk_fw.get(),
        });
        spawn_local(async move {
            match tauri_invoke_args::<Project>("create_project", args).await {
                Ok(p) => {
                    last_created.set(Some(p));
                    if let Ok(list) = tauri_invoke::<Vec<Project>>("list_projects").await {
                        projects.set(list);
                    }
                    right_panel.set(RightPanel::CreateSuccess);
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
        });
    };

    // ── Add file (auto-analyzes SOP if .txt or .pdf) ─────────────────────────
    let on_add_file = move |_| {
        spawn_local(async move {
            let f = match tauri_invoke::<Option<ProjectFile>>("pick_and_add_file").await {
                Ok(Some(f)) => f,
                Ok(None)    => return,
                Err(e)      => { status.set(format!("Error: {e}")); return; }
            };

            let is_sop = f.file_type == "txt" || f.file_type == "pdf";
            refresh_project_data();

            if !is_sop {
                status.set(format!("Added \"{}\"", f.name));
                return;
            }

            // Auto-trigger SOP analysis
            let file_id   = f.id.clone();
            let file_name = f.name.clone();
            sop_analyzing.set(Some(file_id.clone()));
            status.set(format!("Analyzing \"{}\"…", file_name));

            // 1. Read file text
            let text = match tauri_invoke_args::<String>(
                "read_project_file",
                serde_json::json!({ "fileId": file_id.clone() }),
            ).await {
                Ok(t) => t,
                Err(e) => {
                    status.set(format!("Could not read file: {e}"));
                    sop_analyzing.set(None);
                    return;
                }
            };

            // 2. Ask Ollama for structured audit plan JSON
            let prompt = format!("{}\n\n{}", vinrouge::audit_prompts::ANALYZE_SOP, text);
            let json_str = match ask_ollama_json(
                OLLAMA_DEFAULT_URL,
                OLLAMA_DEFAULT_MODEL,
                &prompt,
            ).await {
                Ok(s) => s,
                Err(e) => {
                    status.set(format!("Ollama error: {e}"));
                    sop_analyzing.set(None);
                    return;
                }
            };

            // 3. Save plan to DB
            if let Err(e) = tauri_invoke_args::<()>(
                "save_audit_plan",
                serde_json::json!({
                    "sopFileId":     file_id,
                    "processesJson": json_str,
                }),
            ).await {
                status.set(format!("Save error: {e}"));
                sop_analyzing.set(None);
                return;
            }

            // 4. Reload and display
            if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
                audit_plan.set(p);
            }
            status.set(format!("\"{}\" analyzed — audit plan ready", file_name));
            sop_analyzing.set(None);
        });
    };

    // ── Chat send ─────────────────────────────────────────────────────────────
    let on_chat_send = move |_| {
        let q = chat_input.get();
        if q.trim().is_empty() || chat_loading.get() { return; }
        chat_loading.set(true);
        spawn_local(async move {
            let _ = tauri_invoke_args::<AiMessage>(
                "save_ai_message",
                serde_json::json!({ "role": "user", "content": q.clone() }),
            ).await;
            let reply = match ask_ollama_wasm(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, "", &q).await {
                Ok(r)  => r,
                Err(e) => format!("Error: {e}"),
            };
            let _ = tauri_invoke_args::<AiMessage>(
                "save_ai_message",
                serde_json::json!({ "role": "assistant", "content": reply }),
            ).await;
            chat_input.set(String::new());
            if let Ok(msgs) = tauri_invoke::<Vec<AiMessage>>("list_ai_messages").await {
                ai_messages.set(msgs);
            }
            chat_loading.set(false);
        });
    };

    // ── Validation ────────────────────────────────────────────────────────────
    let step1_valid = move || {
        !wiz_name.get().trim().is_empty()
            && !wiz_client.get().trim().is_empty()
            && !wiz_start.get().is_empty()
            && !wiz_end.get().is_empty()
    };
    let step2_valid = move || !wiz_standards.get().is_empty();

    // ── View ──────────────────────────────────────────────────────────────────
    view! {
        <div
            class=move || if is_dragging.get() { "projects-shell projects-shell--dragging" } else { "projects-shell" }
            on:mousemove=move |ev| {
                if is_dragging.get() {
                    let x = ev.client_x() as f64;
                    sidebar_width.set(x.max(160.0).min(520.0));
                }
            }
            on:mouseup=move   |_| is_dragging.set(false)
            on:mouseleave=move |_| is_dragging.set(false)
        >

            // ── Left sidebar ─────────────────────────────────────────────────
            <div
                class="proj-sidebar"
                style=move || format!("width:{}px;min-width:{}px", sidebar_width.get() as u32, sidebar_width.get() as u32)
            >
                <div class="proj-sidebar-header">
                    <span class="proj-sidebar-title">"PROJECTS"</span>
                    <button class="proj-add-btn" title="New project" on:click=move |_| open_wiz()>
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                            <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                        </svg>
                    </button>
                </div>

                <div class="proj-search-box">
                    <svg class="proj-search-icon" width="13" height="13" viewBox="0 0 13 13" fill="none">
                        <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.2"/>
                        <path d="M9 9l2.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                    </svg>
                    <input
                        class="proj-search-input"
                        type="text"
                        placeholder="Search projects..."
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </div>

                <div class="proj-list">
                    {move || {
                        let q = search.get().to_lowercase();
                        projects.get().into_iter()
                            .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q))
                            .map(|p| {
                                let path      = p.path.clone();
                                let path_del  = p.path.clone();
                                let name      = p.name.clone();
                                let date      = p.created_at.get(..10).unwrap_or("").to_string();
                                let p_id      = p.id.clone();
                                let p_id_del  = p.id.clone();
                                view! {
                                    <div
                                        class=move || {
                                            let active = active_project.get()
                                                .as_ref().map(|a| a.id == p_id).unwrap_or(false);
                                            if active { "proj-list-item proj-list-item--active" }
                                            else      { "proj-list-item" }
                                        }
                                        on:click=move |_| open_project(path.clone())
                                    >
                                        <div class="proj-item-row1">
                                            <span class="proj-item-name">{name}</span>
                                            <button
                                                class="proj-delete-btn"
                                                title="Delete project"
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    let path_d = path_del.clone();
                                                    let pid    = p_id_del.clone();
                                                    spawn_local(async move {
                                                        let args = serde_json::json!({ "path": path_d });
                                                        match tauri_invoke_args::<()>("delete_project", args).await {
                                                            Ok(_) => {
                                                                if active_project.get().as_ref().map(|a| a.id == pid).unwrap_or(false) {
                                                                    active_project.set(None);
                                                                    right_panel.set(RightPanel::Empty);
                                                                }
                                                                if let Ok(list) = tauri_invoke::<Vec<Project>>("list_projects").await {
                                                                    projects.set(list);
                                                                }
                                                            }
                                                            Err(e) => status.set(format!("Delete failed: {e}")),
                                                        }
                                                    });
                                                }
                                            >
                                                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                                    <path d="M2 3h8M5 3V2h2v1M10 3l-.8 7H2.8L2 3"
                                                        stroke="currentColor" stroke-width="1.2"
                                                        stroke-linecap="round" stroke-linejoin="round"/>
                                                </svg>
                                            </button>
                                        </div>
                                        <div class="proj-item-meta">{date}</div>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>

                <button class="proj-new-btn-dashed" on:click=move |_| open_wiz()>
                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                        <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                    "New project"
                </button>
            </div>

            // ── Resize handle ─────────────────────────────────────────────────
            <div
                class=move || if is_dragging.get() { "proj-resize-handle proj-resize-handle--active" } else { "proj-resize-handle" }
                on:mousedown=move |ev| { ev.prevent_default(); is_dragging.set(true); }
            />

            // ── Right panel ───────────────────────────────────────────────────
            <div class="proj-main">

                // Status toast
                {move || {
                    let s = status.get();
                    (!s.is_empty()).then(|| view! { <div class="proj-status-toast">{s}</div> })
                }}

                // ── Empty state ───────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::Empty).then(|| view! {
                    <div class="proj-empty-state">
                        <div class="proj-empty-icon">
                            <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
                                <path d="M13 3L4 7.5v11L13 23l9-4.5v-11L13 3z"
                                    stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                                <path d="M13 3v20M4 7.5l9 5 9-5" stroke="currentColor" stroke-width="1.3"/>
                            </svg>
                        </div>
                        <div class="proj-empty-title">"No project selected"</div>
                        <div class="proj-empty-sub">
                            "Select an existing project from the sidebar, or create a new one to get started."
                        </div>
                        <button class="wiz-btn-primary" on:click=move |_| open_wiz()>
                            "Create new project"
                        </button>
                    </div>
                })}

                // ── Create wizard ─────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::CreateWizard).then(|| view! {
                    <div class="wiz-container">

                        // Step bar (3 steps)
                        <div class="wiz-step-bar">
                            {move || {
                                let cur = wiz_step.get();
                                let steps: &[(&str, &str)] = &[
                                    ("1", "Project details"),
                                    ("2", "Standards & scope"),
                                    ("3", "Review & create"),
                                ];
                                steps.iter().enumerate().map(|(i, &(num, label))| {
                                    let idx = (i + 1) as u8;
                                    let cls = if cur == idx      { "wiz-step wiz-step--active" }
                                              else if cur > idx  { "wiz-step wiz-step--done"   }
                                              else               { "wiz-step"                   };
                                    view! {
                                        <>
                                            {(i > 0).then(|| view! { <div class="wiz-step-line"></div> })}
                                            <div class=cls>
                                                <div class="wiz-step-num">{num}</div>
                                                <span class="wiz-step-label">{label}</span>
                                            </div>
                                        </>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        // Step panels
                        {move || {
                            let step = wiz_step.get();

                            // ── Step 1: Project details ───────────────────────
                            if step == 1 { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Project details"</div>
                                        <div class="wiz-sub">"Basic information about this audit engagement"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div>
                                            <div class="wiz-section-label">"ENGAGEMENT"</div>
                                            <div style="display:flex;flex-direction:column;gap:12px">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Project name "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="text"
                                                        placeholder="e.g. Acme Corp — ISO 27001 Audit 2026"
                                                        prop:value=move || wiz_name.get()
                                                        on:input=move |ev| wiz_name.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-two-col">
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">
                                                            "Client / entity name "
                                                            <span class="wiz-req">"*"</span>
                                                        </label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="Client name"
                                                            prop:value=move || wiz_client.get()
                                                            on:input=move |ev| wiz_client.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Engagement reference"</label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="e.g. ENG-2026-041"
                                                            prop:value=move || wiz_ref_.get()
                                                            on:input=move |ev| wiz_ref_.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"AUDIT PERIOD"</div>
                                            <div class="wiz-three-col">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Period start "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_start.get()
                                                        on:input=move |ev| wiz_start.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-field">
                                                    <label class="wiz-label">
                                                        "Period end "
                                                        <span class="wiz-req">"*"</span>
                                                    </label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_end.get()
                                                        on:input=move |ev| wiz_end.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="wiz-field">
                                                    <label class="wiz-label">"Report due date"</label>
                                                    <input class="wiz-input" type="date"
                                                        prop:value=move || wiz_due.get()
                                                        on:input=move |ev| wiz_due.set(event_target_value(&ev))
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"AUDIT TYPE"</div>
                                            <div class="wiz-audit-cards">
                                                {["Compliance", "Financial", "Operational"].into_iter().map(|t| view! {
                                                    <div
                                                        class=move || if wiz_type.get() == t {
                                                            "wiz-audit-card wiz-audit-card--selected"
                                                        } else { "wiz-audit-card" }
                                                        on:click=move |_| wiz_type.set(t.to_string())
                                                    >
                                                        <div class="wiz-audit-name">{t}</div>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"NOTES"</div>
                                            <textarea class="wiz-textarea"
                                                placeholder="Optional — scope limitations, special instructions..."
                                                prop:value=move || wiz_notes.get()
                                                on:input=move |ev| wiz_notes.set(event_target_value(&ev))
                                            ></textarea>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"SAVE LOCATION"</div>
                                            <div class="wiz-folder-row">
                                                <span class="wiz-folder-path">
                                                    {move || wiz_save_dir.get()
                                                        .unwrap_or_else(|| "~/VinRouge/projects (default)".to_string())}
                                                </span>
                                                <button class="wiz-btn-ghost" on:click=on_pick_folder>
                                                    "Browse…"
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            {move || if step1_valid() {
                                                "Ready to continue"
                                            } else {
                                                "Fill in required fields to continue"
                                            }}
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| right_panel.set(RightPanel::Empty)>
                                                "Cancel"
                                            </button>
                                            <button class="wiz-btn-primary"
                                                disabled=move || !step1_valid()
                                                on:click=move |_| wiz_step.set(2)>
                                                "Next — Standards"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()

                            // ── Step 2: Standards & scope ─────────────────────
                            } else if step == 2 { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Standards & scope"</div>
                                        <div class="wiz-sub">"Select the applicable standards and define the audit scope"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div>
                                            <div class="wiz-section-label">"APPLICABLE STANDARDS"</div>
                                            <div class="wiz-standards-grid">
                                                {["ISO 27001", "SOC 2", "GDPR", "GAAP", "ISA 315", "SOX"]
                                                    .into_iter().map(|s| view! {
                                                    <div
                                                        class=move || if wiz_standards.get().contains(&s.to_string()) {
                                                            "wiz-standard-item wiz-standard-item--selected"
                                                        } else { "wiz-standard-item" }
                                                        on:click=move |_| {
                                                            wiz_standards.update(|v| {
                                                                let name = s.to_string();
                                                                if v.contains(&name) { v.retain(|x| x != &name); }
                                                                else { v.push(name); }
                                                            });
                                                        }
                                                    >
                                                        <div class=move || if wiz_standards.get().contains(&s.to_string()) {
                                                            "wiz-std-chk wiz-std-chk--checked"
                                                        } else { "wiz-std-chk" }>
                                                            {move || wiz_standards.get().contains(&s.to_string()).then(|| view! {
                                                                <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
                                                                    <path d="M1 4l3 3 5-6" stroke="white"
                                                                        stroke-width="1.5" stroke-linecap="round"
                                                                        stroke-linejoin="round"/>
                                                                </svg>
                                                            })}
                                                        </div>
                                                        <div class="wiz-std-name">{s}</div>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        </div>
                                        <div>
                                            <div class="wiz-section-label">"SCOPE STATEMENT"</div>
                                            <div style="display:flex;flex-direction:column;gap:12px">
                                                <div class="wiz-field">
                                                    <label class="wiz-label">"In-scope systems & processes"</label>
                                                    <textarea class="wiz-textarea"
                                                        placeholder="Describe the systems, processes, or business units within scope..."
                                                        prop:value=move || wiz_scope.get()
                                                        on:input=move |ev| wiz_scope.set(event_target_value(&ev))
                                                    ></textarea>
                                                </div>
                                                <div class="wiz-two-col">
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Materiality threshold"</label>
                                                        <input class="wiz-input" type="text"
                                                            placeholder="e.g. R50,000 / 5% of revenue"
                                                            prop:value=move || wiz_materiality.get()
                                                            on:input=move |ev| wiz_materiality.set(event_target_value(&ev))
                                                        />
                                                    </div>
                                                    <div class="wiz-field">
                                                        <label class="wiz-label">"Risk rating framework"</label>
                                                        <select class="wiz-input"
                                                            prop:value=move || wiz_risk_fw.get()
                                                            on:change=move |ev| wiz_risk_fw.set(event_target_value(&ev))
                                                        >
                                                            <option value="High / Medium / Low">"High / Medium / Low"</option>
                                                            <option value="1–5 numeric scale">"1–5 numeric scale"</option>
                                                            <option value="Red / Amber / Green">"Red / Amber / Green"</option>
                                                            <option value="Custom">"Custom"</option>
                                                        </select>
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            {move || {
                                                let stds = wiz_standards.get();
                                                if stds.is_empty() {
                                                    "Select at least one standard".to_string()
                                                } else {
                                                    format!("{} selected", stds.join(", "))
                                                }
                                            }}
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| wiz_step.set(1)>"Back"</button>
                                            <button class="wiz-btn-primary"
                                                disabled=move || !step2_valid()
                                                on:click=move |_| wiz_step.set(3)>
                                                "Review & create"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()

                            // ── Step 3: Review & create ───────────────────────
                            } else { view! {
                                <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                                    <div class="wiz-header">
                                        <div class="wiz-title">"Review & create"</div>
                                        <div class="wiz-sub">"Confirm all details before creating the project"</div>
                                    </div>
                                    <div class="wiz-body">
                                        <div class="wiz-review-alert">
                                            <svg width="15" height="15" viewBox="0 0 15 15" fill="none"
                                                style="flex-shrink:0;margin-top:1px">
                                                <circle cx="7.5" cy="7.5" r="6.5"
                                                    stroke="currentColor" stroke-width="1.1"/>
                                                <path d="M7.5 5v3.5M7.5 10.5h.01"
                                                    stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
                                            </svg>
                                            "A project record and empty audit trail log will be created on-device. No data leaves this machine."
                                        </div>

                                        // Project details card
                                        <div class="wiz-review-card">
                                            <div class="wiz-review-card-head">
                                                "Project details"
                                                <button class="wiz-review-edit"
                                                    on:click=move |_| wiz_step.set(1)>"Edit"</button>
                                            </div>
                                            <div class="wiz-review-grid">
                                                <div>
                                                    <div class="wiz-rv-label">"Project name"</div>
                                                    <div class="wiz-rv-val">{move || wiz_name.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Client"</div>
                                                    <div class="wiz-rv-val">{move || wiz_client.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Period"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || format!("{} → {}", wiz_start.get(), wiz_end.get())}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Audit type"</div>
                                                    <div class="wiz-rv-val">{move || wiz_type.get()}</div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Reference"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let r = wiz_ref_.get();
                                                            if r.is_empty() { "—".to_string() } else { r }
                                                        }}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Report due"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let d = wiz_due.get();
                                                            if d.is_empty() { "—".to_string() } else { d }
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                        </div>

                                        // Standards & scope card
                                        <div class="wiz-review-card">
                                            <div class="wiz-review-card-head">
                                                "Standards & scope"
                                                <button class="wiz-review-edit"
                                                    on:click=move |_| wiz_step.set(2)>"Edit"</button>
                                            </div>
                                            <div class="wiz-review-tags">
                                                {move || wiz_standards.get().into_iter()
                                                    .map(|s| view! { <span class="wiz-std-tag">{s}</span> })
                                                    .collect_view()}
                                            </div>
                                            <div class="wiz-review-grid" style="padding-top:0">
                                                <div>
                                                    <div class="wiz-rv-label">"Materiality threshold"</div>
                                                    <div class="wiz-rv-val">
                                                        {move || {
                                                            let m = wiz_materiality.get();
                                                            if m.is_empty() { "—".to_string() } else { m }
                                                        }}
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="wiz-rv-label">"Risk rating framework"</div>
                                                    <div class="wiz-rv-val">{move || wiz_risk_fw.get()}</div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="wiz-footer">
                                        <span class="wiz-validation-msg">
                                            "Project will be saved locally — no data leaves this machine"
                                        </span>
                                        <div class="wiz-footer-actions">
                                            <button class="wiz-btn-ghost"
                                                on:click=move |_| wiz_step.set(2)>"Back"</button>
                                            <button class="wiz-btn-primary" on:click=on_create>
                                                "Create project"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }.into_any() }
                        }}

                    </div> // wiz-container
                })}

                // ── Success screen ────────────────────────────────────────────
                {move || (right_panel.get() == RightPanel::CreateSuccess).then(|| {
                    let pname = last_created.get().as_ref().map(|p| p.name.clone()).unwrap_or_default();
                    let stds  = wiz_standards.get().join(", ");
                    let stds2 = if stds.is_empty() { "—".to_string() } else { stds };
                    view! {
                        <div class="proj-success">
                            <div class="proj-success-icon">
                                <svg width="26" height="26" viewBox="0 0 28 28" fill="none">
                                    <path d="M5 14l7 7 11-11" stroke="currentColor"
                                        stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                            </div>
                            <div class="proj-success-title">"Project created"</div>
                            <div class="proj-success-sub">
                                "Your project has been created and the audit trail log initialised."
                            </div>
                            <div class="proj-success-meta">
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Client"</div>
                                    <div class="proj-smi-val">{move || wiz_client.get()}</div>
                                </div>
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Standards"</div>
                                    <div class="proj-smi-val">{stds2}</div>
                                </div>
                                <div class="proj-success-meta-item">
                                    <div class="proj-smi-label">"Status"</div>
                                    <div class="proj-smi-val proj-smi-val--accent">"Draft"</div>
                                </div>
                            </div>
                            <div style="display:flex;gap:10px">
                                <button class="wiz-btn-ghost"
                                    on:click=move |_| right_panel.set(RightPanel::Empty)>
                                    "Back to projects"
                                </button>
                                <button class="wiz-btn-primary"
                                    on:click=move |_| {
                                        if let Some(p) = last_created.get() {
                                            open_project(p.path.clone());
                                        }
                                    }>
                                    "Open project"
                                </button>
                            </div>
                        </div>
                    }
                })}

                // ── Active project panel ──────────────────────────────────────
                {move || (right_panel.get() == RightPanel::ActiveProject).then(|| {
                    let pname = active_project.get().as_ref().map(|p| p.name.clone()).unwrap_or_default();
                    view! {
                        <div class="proj-active-panel">
                            <div class="proj-active-header">
                                <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                                    <path d="M8 1.5L2 4.5v7L8 14.5l6-3v-7L8 1.5z"
                                        stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
                                </svg>
                                <span style="font-size:14px;font-weight:500">{pname}</span>
                                <button class="wiz-btn-primary"
                                    style="margin-left:auto;padding:5px 14px;font-size:12px"
                                    disabled=move || sop_analyzing.get().is_some()
                                    on:click=on_add_file>
                                    {move || if sop_analyzing.get().is_some() {
                                        "Analyzing…"
                                    } else {
                                        "+ Add File"
                                    }}
                                </button>
                            </div>

                            <div class="proj-active-files">
                                <div class="wiz-section-label">"FILES"</div>
                                {move || {
                                    let files = project_files.get();
                                    if files.is_empty() {
                                        view! {
                                            <p class="proj-empty-note">
                                                "No files yet — click Add File to upload data."
                                            </p>
                                        }.into_any()
                                    } else {
                                        files.into_iter().map(|f| {
                                            let file_id       = f.id.clone();
                                            let file_id_dis   = f.id.clone();
                                            let file_id_label = f.id.clone();
                                            let file_id2      = f.id.clone();
                                            let file_type = f.file_type.clone();
                                            let is_sop = file_type == "txt" || file_type == "pdf";
                                            view! {
                                                <div class="proj-file-item">
                                                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                                                        <path d="M3 2h6l3 3v7a1 1 0 01-1 1H3a1 1 0 01-1-1V3a1 1 0 011-1z"
                                                            stroke="currentColor" stroke-width="1.1"/>
                                                        <path d="M9 2v3h3" stroke="currentColor" stroke-width="1.1"/>
                                                    </svg>
                                                    <span class="proj-file-name">{f.name}</span>
                                                    <span class="proj-file-type">{f.file_type}</span>
                                                    {is_sop.then(|| view! {
                                                        <button
                                                            class="sop-analyze-btn"
                                                            disabled=move || sop_analyzing.get().as_deref() == Some(&file_id_dis)
                                                            on:click=move |_| {
                                                                let fid = file_id2.clone();
                                                                sop_analyzing.set(Some(fid.clone()));
                                                                status.set(String::new());
                                                                spawn_local(async move {
                                                                    // 1. Read file text via Tauri
                                                                    let text = match tauri_invoke_args::<String>(
                                                                        "read_project_file",
                                                                        serde_json::json!({ "fileId": fid.clone() }),
                                                                    ).await {
                                                                        Ok(t) => t,
                                                                        Err(e) => {
                                                                            status.set(format!("Read error: {e}"));
                                                                            sop_analyzing.set(None);
                                                                            return;
                                                                        }
                                                                    };

                                                                    // 2. Build prompt and ask Ollama for JSON plan
                                                                    let prompt = format!(
                                                                        "{}\n\n{}",
                                                                        vinrouge::audit_prompts::ANALYZE_SOP,
                                                                        text
                                                                    );
                                                                    let json_str = match ask_ollama_json(
                                                                        OLLAMA_DEFAULT_URL,
                                                                        OLLAMA_DEFAULT_MODEL,
                                                                        &prompt,
                                                                    ).await {
                                                                        Ok(s) => s,
                                                                        Err(e) => {
                                                                            status.set(format!("Ollama error: {e}"));
                                                                            sop_analyzing.set(None);
                                                                            return;
                                                                        }
                                                                    };

                                                                    // 3. Save plan to DB via Tauri
                                                                    if let Err(e) = tauri_invoke_args::<()>(
                                                                        "save_audit_plan",
                                                                        serde_json::json!({
                                                                            "sopFileId":    fid,
                                                                            "processesJson": json_str,
                                                                        }),
                                                                    ).await {
                                                                        status.set(format!("Save error: {e}"));
                                                                        sop_analyzing.set(None);
                                                                        return;
                                                                    }

                                                                    // 4. Reload audit plan
                                                                    if let Ok(p) = tauri_invoke::<Vec<AuditProcessWithControls>>("list_audit_plan").await {
                                                                        audit_plan.set(p);
                                                                    }
                                                                    sop_analyzing.set(None);
                                                                });
                                                            }
                                                        >
                                                            {move || if sop_analyzing.get().as_deref() == Some(&file_id_label) {
                                                                "Analyzing…"
                                                            } else {
                                                                "Re-analyze"
                                                            }}
                                                        </button>
                                                    })}
                                                </div>
                                            }
                                        }).collect_view().into_any()
                                    }
                                }}
                            </div>

                            // ── Audit plan ────────────────────────────────────
                            {move || {
                                let plan = audit_plan.get();
                                (!plan.is_empty()).then(|| view! {
                                    <AuditPlanView plan=plan />
                                })
                            }}

                            <div class="proj-active-chat">
                                <div class="wiz-section-label" style="margin-bottom:8px">"CHAT"</div>
                                <div class="chat-messages">
                                    {move || ai_messages.get().into_iter().map(|m| {
                                        let role    = m.role.clone();
                                        let role2   = m.role.clone();
                                        let content = m.content.clone();
                                        view! {
                                            <div class=format!("chat-msg chat-msg-{role}")>
                                                <span class="chat-role">{role2}</span>
                                                <pre class="chat-content">{content}</pre>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                                <div class="ollama-query-row">
                                    <textarea
                                        class="ollama-textarea"
                                        rows="2"
                                        placeholder="Ask about this project…"
                                        prop:value=move || chat_input.get()
                                        on:input=move |ev| chat_input.set(event_target_value(&ev))
                                    />
                                    <button
                                        class="wiz-btn-primary"
                                        on:click=on_chat_send
                                        disabled=move || chat_loading.get()
                                    >
                                        {move || if chat_loading.get() { "Thinking…" } else { "Send" }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                })}

            </div> // proj-main
        </div> // projects-shell
    }
}

// ── AuditPlanView ──────────────────────────────────────────────────────────────

#[component]
fn AuditPlanView(plan: Vec<AuditProcessWithControls>) -> impl IntoView {
    let count = plan.len();
    view! {
        <div class="audit-plan">
            <div class="audit-plan-header">
                <span class="audit-plan-title">"AUDIT PLAN"</span>
                <span class="audit-plan-badge">{count} " processes"</span>
            </div>
            {plan.into_iter().map(|proc| view! {
                <AuditProcessCard proc=proc />
            }).collect_view()}
        </div>
    }
}

#[component]
fn AuditProcessCard(proc: AuditProcessWithControls) -> impl IntoView {
    let open: RwSignal<bool> = RwSignal::new(true);

    let proc_id   = proc.id.clone();
    let proc_id2  = proc.id.clone();
    let pname_sig: RwSignal<String>  = RwSignal::new(proc.process_name.clone());
    let pdesc_sig: RwSignal<String>  = RwSignal::new(proc.description.clone());
    let controls  = proc.controls.clone();

    // editing state for process fields
    let edit_name: RwSignal<bool> = RwSignal::new(false);
    let edit_desc: RwSignal<bool> = RwSignal::new(false);

    view! {
        <div class="audit-process-card">
            <div class="audit-process-header" on:click=move |_| open.update(|v| *v = !*v)>
                <span class=move || if open.get() {
                    "audit-process-chevron open"
                } else {
                    "audit-process-chevron"
                }>"▶"</span>
                <div class="audit-process-name-wrap">
                    // Process name — click to edit
                    {move || if edit_name.get() {
                        let pid = proc_id.clone();
                        view! {
                            <input
                                class="editable-input"
                                prop:value=move || pname_sig.get()
                                on:input=move |ev| pname_sig.set(event_target_value(&ev))
                                on:blur=move |_| {
                                    edit_name.set(false);
                                    let v = pname_sig.get();
                                    let pid2 = pid.clone();
                                    spawn_local(async move {
                                        let _ = tauri_invoke_args::<()>(
                                            "update_process_field",
                                            serde_json::json!({
                                                "processId": pid2,
                                                "field": "process_name",
                                                "value": v,
                                            }),
                                        ).await;
                                    });
                                }
                                on:click=move |ev| { ev.stop_propagation(); }
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="audit-process-name"
                                on:click=move |ev| { ev.stop_propagation(); edit_name.set(true); }>
                                {move || pname_sig.get()}
                            </div>
                        }.into_any()
                    }}
                    // Process description — click to edit
                    {move || if edit_desc.get() {
                        let pid = proc_id2.clone();
                        view! {
                            <input
                                class="editable-input"
                                style="margin-top:2px"
                                prop:value=move || pdesc_sig.get()
                                on:input=move |ev| pdesc_sig.set(event_target_value(&ev))
                                on:blur=move |_| {
                                    edit_desc.set(false);
                                    let v = pdesc_sig.get();
                                    let pid2 = pid.clone();
                                    spawn_local(async move {
                                        let _ = tauri_invoke_args::<()>(
                                            "update_process_field",
                                            serde_json::json!({
                                                "processId": pid2,
                                                "field": "description",
                                                "value": v,
                                            }),
                                        ).await;
                                    });
                                }
                                on:click=move |ev| { ev.stop_propagation(); }
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="audit-process-desc"
                                on:click=move |ev| { ev.stop_propagation(); edit_desc.set(true); }>
                                {move || pdesc_sig.get()}
                            </div>
                        }.into_any()
                    }}
                </div>
            </div>

            {move || open.get().then(|| {
                let rows = controls.clone();
                view! {
                    <div class="audit-process-body">
                        {if rows.is_empty() {
                            view! {
                                <p class="audit-empty">"No controls — re-analyze to generate."</p>
                            }.into_any()
                        } else {
                            view! {
                                <table class="controls-table">
                                    <thead>
                                        <tr>
                                            <th>"Ref"</th>
                                            <th>"Control objective"</th>
                                            <th>"How it operates"</th>
                                            <th>"Test procedure"</th>
                                            <th>"Risk"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {rows.into_iter().map(|c| view! {
                                            <ControlRow ctrl=c />
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            }.into_any()
                        }}
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn ControlRow(ctrl: Control) -> impl IntoView {
    let cid_ref  = ctrl.id.clone();
    let cid_obj  = ctrl.id.clone();
    let cid_desc = ctrl.id.clone();
    let cid_test = ctrl.id.clone();
    let cid_risk = ctrl.id.clone();

    let ref_sig:  RwSignal<String> = RwSignal::new(ctrl.control_ref.clone());
    let obj_sig:  RwSignal<String> = RwSignal::new(ctrl.control_objective.clone());
    let desc_sig: RwSignal<String> = RwSignal::new(ctrl.control_description.clone());
    let test_sig: RwSignal<String> = RwSignal::new(ctrl.test_procedure.clone());
    let risk_sig: RwSignal<String> = RwSignal::new(ctrl.risk_level.clone());

    let risk_class = move || match risk_sig.get().as_str() {
        "High" => "risk-badge high",
        "Low"  => "risk-badge low",
        _      => "risk-badge medium",
    };

    view! {
        <tr>
            // Ref
            <td class="ctrl-ref">
                <input class="editable-input"
                    style="width:46px"
                    prop:value=move || ref_sig.get()
                    on:input=move |ev| ref_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = ref_sig.get();
                        let id = cid_ref.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_ref","value":v})).await;
                        });
                    }
                />
            </td>
            // Objective
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || obj_sig.get()
                    on:input=move |ev| obj_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = obj_sig.get();
                        let id = cid_obj.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_objective","value":v})).await;
                        });
                    }
                />
            </td>
            // Description
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || desc_sig.get()
                    on:input=move |ev| desc_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = desc_sig.get();
                        let id = cid_desc.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"control_description","value":v})).await;
                        });
                    }
                />
            </td>
            // Test procedure
            <td class="editable-cell">
                <textarea class="editable-textarea"
                    prop:value=move || test_sig.get()
                    on:input=move |ev| test_sig.set(event_target_value(&ev))
                    on:blur=move |_| {
                        let v = test_sig.get();
                        let id = cid_test.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"test_procedure","value":v})).await;
                        });
                    }
                />
            </td>
            // Risk level
            <td class="ctrl-risk">
                <select class="risk-select"
                    prop:value=move || risk_sig.get()
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        risk_sig.set(v.clone());
                        let id = cid_risk.clone();
                        spawn_local(async move {
                            let _ = tauri_invoke_args::<()>("update_control_field",
                                serde_json::json!({"controlId":id,"field":"risk_level","value":v})).await;
                        });
                    }
                >
                    <option value="High">"High"</option>
                    <option value="Medium">"Medium"</option>
                    <option value="Low">"Low"</option>
                </select>
                <span class=risk_class>{move || risk_sig.get()}</span>
            </td>
        </tr>
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

// ── Ollama JSON helper (SOP analysis) ─────────────────────────────────────────

/// Sends `prompt` to Ollama with `"format":"json"` and returns the raw JSON string.
async fn ask_ollama_json(base_url: &str, model: &str, prompt: &str) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
        "format": "json"
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let resp = Request::post(&url)
        .json(&body)
        .map_err(|e| format!("Failed to build request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Could not reach Ollama at {url}: {e}"))?;

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
        s.push_str(&format!(
            "\nRelationships ({}):\n",
            result.relationships.len()
        ));
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
