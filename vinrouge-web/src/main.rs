pub mod components;
mod file_analysis;
mod ipc;
mod ollama;
mod projects;
mod studio;
mod step1;
mod step2;
mod step3;
mod step4;
mod step4a;
mod step4b;
mod step5;
mod step5a;
mod step5b;
mod storage;
mod types;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use components::Spinner;
use components::OllamaSection;
use ipc::{
    is_tauri, tauri_check_model, tauri_listen_pull_progress, tauri_pull_model,
};
use projects::ProjectsView;
use studio::StudioView;

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
    let active_tab: RwSignal<&'static str> = RwSignal::new("studio");
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

    let subtitle = if tauri {
        "Standalone desktop app"
    } else {
        "Data analysis platform"
    };

    view! {
        <header>
            <h1>"VinRouge"</h1>
            <nav class="top-nav">
                <button
                    class=move || if active_tab.get() == "studio" { "nav-btn active" } else { "nav-btn" }
                    on:click=move |_| active_tab.set("studio")
                >"Studio"</button>
                <button
                    class=move || if active_tab.get() == "chat" { "nav-btn active" } else { "nav-btn" }
                    on:click=move |_| active_tab.set("chat")
                >"Chat"</button>
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

        <main class=move || {
            let tab = active_tab.get();
            if tab == "projects" || tab == "studio" { "projects-active" } else { "" }
        }>
            // ── Studio tab ────────────────────────────────────────────────────
            {move || (active_tab.get() == "studio").then(|| view! {
                <StudioView />
            })}

            // ── Chat tab ──────────────────────────────────────────────────────
            {move || (active_tab.get() == "chat").then(|| view! {
                <OllamaSection summary=move || String::new() />
            })}

            // ── Projects tab ──────────────────────────────────────────────────
            {move || (active_tab.get() == "projects").then(|| view! {
                <ProjectsView />
            })}
        </main>
    }
}
