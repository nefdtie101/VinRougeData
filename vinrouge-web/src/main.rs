pub mod components;
mod data_view;
mod file_analysis;
mod ipc;
mod ollama;
mod projects;
mod step1;
mod step2;
mod step3;
mod step4;
mod step4a;
mod step4b;
mod step5;
mod storage;
mod types;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use components::Spinner;
use data_view::{OllamaSection, Results};
use file_analysis::{analyze_bytes, read_file_bytes};
use ipc::{
    is_tauri, tauri_check_model, tauri_listen_pull_progress, tauri_pick_and_analyze,
    tauri_pull_model,
};
use ollama::build_web_summary;
use projects::ProjectsView;
use types::AnalysisResult;

#[derive(Clone, PartialEq)]
enum ModelState {
    Checking,
    Pulling(u8, String), // percent 0-100, status message
    Ready,
    Error(String),
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> impl IntoView {
    let result: RwSignal<Option<AnalysisResult>> = RwSignal::new(None);
    let status: RwSignal<String> = RwSignal::new(String::new());
    let loading: RwSignal<bool> = RwSignal::new(false);
    let active_tab: RwSignal<&'static str> = RwSignal::new("chat");
    let tauri = is_tauri();

    // ── Model availability check (Tauri only) ─────────────────────────────────
    let model_state: RwSignal<ModelState> = RwSignal::new(if tauri {
        ModelState::Checking
    } else {
        ModelState::Ready
    });
    Effect::new(move |_: Option<()>| {
        if !tauri {
            return;
        }
        spawn_local(async move {
            match tauri_check_model().await {
                Ok(true) => model_state.set(ModelState::Ready),
                Ok(false) => {
                    model_state.set(ModelState::Pulling(0, "Starting\u{2026}".into()));

                    // Stream progress events into the signal
                    let _ = tauri_listen_pull_progress(move |percent, status, done| {
                        if done {
                            model_state.set(ModelState::Ready);
                        } else {
                            model_state.set(ModelState::Pulling(percent, status));
                        }
                    });

                    // Also await the command so errors surface correctly
                    if let Err(e) = tauri_pull_model().await {
                        model_state.set(ModelState::Error(e));
                    }
                }
                Err(e) => model_state.set(ModelState::Error(e)),
            }
        });
    });

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
        let input: web_sys::HtmlInputElement = ev.target().unwrap().unchecked_into();
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

        // ── Model pull banner ─────────────────────────────────────────────────
        {move || match model_state.get() {
            ModelState::Ready => None,
            ModelState::Checking => Some(view! {
                <div class="model-pull-banner">
                    <Spinner size=14 />
                    "Checking AI model\u{2026}"
                </div>
            }.into_any()),
            ModelState::Pulling(percent, status) => Some(view! {
                <div class="model-pull-banner">
                    <Spinner size=14 />
                    <span class="model-pull-label">
                        "Downloading Mistral \u{2014} "
                        {status}
                    </span>
                    <div class="model-pull-track">
                        <div
                            class="model-pull-fill"
                            style=format!("width:{percent}%")
                        />
                    </div>
                    <span class="model-pull-pct">{percent} "%"</span>
                </div>
            }.into_any()),
            ModelState::Error(e) => Some(view! {
                <div class="model-pull-banner model-pull-banner--error">
                    "AI model error: " {e}
                </div>
            }.into_any()),
        }}

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
