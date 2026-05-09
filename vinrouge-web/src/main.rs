pub mod components;
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
mod step5a;
mod step5b;
mod storage;
mod studio;
mod types;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use components::OllamaSection;
use components::Spinner;
use ipc::{
    is_tauri, tauri_check_model, tauri_check_update, tauri_download_update, tauri_install_update,
    tauri_listen_event_payload, tauri_listen_pull_progress, tauri_pull_model,
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

#[derive(Clone, PartialEq)]
enum UpdateState {
    Checking,
    Available {
        current: String,
        latest: String,
        url: String,
        name: String,
    },
    Downloading(u8, String),
    Installing,
    UpToDate,
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

    // ── Update check (Tauri only) ─────────────────────────────────────────────
    let update_state: RwSignal<UpdateState> = RwSignal::new(if tauri {
        UpdateState::Checking
    } else {
        UpdateState::UpToDate
    });
    Effect::new(move |_: Option<()>| {
        if !tauri {
            return;
        }
        spawn_local(async move {
            // Listen for download progress events
            let _ = tauri_listen_event_payload("update-progress", move |payload| {
                let percent = payload.get("percent").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                let status = payload
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Downloading...")
                    .to_string();
                let done = payload
                    .get("done")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if done {
                    update_state.set(UpdateState::Installing);
                } else {
                    update_state.set(UpdateState::Downloading(percent, status));
                }
            })
            .await;

            match tauri_check_update().await {
                Ok(None) => update_state.set(UpdateState::UpToDate),
                Ok(Some(info)) => update_state.set(UpdateState::Available {
                    current: info.current_version,
                    latest: info.latest_version,
                    url: info.download_url,
                    name: info.asset_name,
                }),
                Err(e) => {
                    // Silently ignore errors so a network hiccup doesn't spam the UI
                    update_state.set(UpdateState::UpToDate);
                }
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

        // ── Update banner ─────────────────────────────────────────────────────
        {move || match update_state.get() {
            UpdateState::Checking => None,
            UpdateState::UpToDate => None,
            UpdateState::Available { current, latest, url, name } => Some(view! {
                <div class="update-banner">
                    <span class="update-banner-text">
                        "Update available: " {current} " \u{2192} " {latest}
                    </span>
                    <button
                        class="update-banner-btn"
                        on:click=move |_| {
                            let url = url.clone();
                            update_state.set(UpdateState::Downloading(0, "Starting...".into()));
                            spawn_local(async move {
                                match tauri_download_update(url).await {
                                    Ok(path) => {
                                        if let Err(e) = tauri_install_update(path).await {
                                            update_state.set(UpdateState::Error(e));
                                        }
                                    }
                                    Err(e) => update_state.set(UpdateState::Error(e)),
                                }
                            });
                        }
                    >"Download & Install"</button>
                </div>
            }.into_any()),
            UpdateState::Downloading(percent, status) => Some(view! {
                <div class="update-banner">
                    <Spinner size=14 />
                    <span class="update-banner-text">{status}</span>
                    <div class="model-pull-track">
                        <div class="model-pull-fill" style=format!("width:{percent}%") />
                    </div>
                    <span class="model-pull-pct">{percent} "%"</span>
                </div>
            }.into_any()),
            UpdateState::Installing => Some(view! {
                <div class="update-banner">
                    <Spinner size=14 />
                    <span class="update-banner-text">"Installing update... Please restart the app when prompted."</span>
                </div>
            }.into_any()),
            UpdateState::Error(e) => Some(view! {
                <div class="update-banner update-banner--error">
                    "Update failed: " {e}
                </div>
            }.into_any()),
        }}

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
