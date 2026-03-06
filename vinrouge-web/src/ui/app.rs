use leptos::*;
use std::rc::Rc;
use vinrouge::export::AnalysisResult;

use super::{export_bar::ExportBar, results_view::ResultsView, upload_panel::UploadPanel};

#[derive(Clone)]
pub enum AppState {
    Idle,
    Analyzing,
    Done(Rc<AnalysisResult>),
    Error(String),
}

#[component]
pub fn App() -> impl IntoView {
    let (state, set_state) = create_signal(AppState::Idle);

    view! {
        <div class="app">
            <header class="app-header">
                <h1 class="app-title">"VinRouge"</h1>
                <p class="app-subtitle">"Data Analysis — Web Edition"</p>
            </header>

            <main class="app-main">
                {move || match state.get() {
                    AppState::Idle | AppState::Analyzing => {
                        let is_loading = matches!(state.get(), AppState::Analyzing);
                        view! {
                            <UploadPanel
                                is_loading=is_loading
                                set_state=set_state
                            />
                        }.into_view()
                    }
                    AppState::Done(result) => {
                        view! {
                            <div class="results-container">
                                <div class="results-toolbar">
                                    <button
                                        class="btn btn-secondary"
                                        on:click=move |_| set_state.set(AppState::Idle)
                                    >
                                        "← Upload new files"
                                    </button>
                                    <ExportBar result=result.clone() />
                                </div>
                                <ResultsView result=result />
                            </div>
                        }.into_view()
                    }
                    AppState::Error(msg) => {
                        view! {
                            <div class="error-box">
                                <h2>"Analysis failed"</h2>
                                <pre class="error-msg">{msg}</pre>
                                <button
                                    class="btn btn-primary"
                                    on:click=move |_| set_state.set(AppState::Idle)
                                >
                                    "Try again"
                                </button>
                            </div>
                        }.into_view()
                    }
                }}
            </main>

            <footer class="app-footer">
                <p>"VinRouge Data Analysis — running entirely in your browser via WebAssembly"</p>
            </footer>
        </div>
    }
}
