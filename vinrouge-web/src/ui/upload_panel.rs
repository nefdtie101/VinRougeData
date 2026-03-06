use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;

use super::app::AppState;
use crate::{
    analysis_bridge::{run_analysis, UploadedFile},
    file_upload::read_file_as_bytes,
};

#[component]
pub fn UploadPanel(is_loading: bool, set_state: WriteSignal<AppState>) -> impl IntoView {
    let on_files_selected = move |files: Vec<web_sys::File>| {
        if files.is_empty() {
            return;
        }
        set_state.set(AppState::Analyzing);
        spawn_local(async move {
            let mut uploaded = Vec::new();
            for file in files {
                match read_file_as_bytes(&file).await {
                    Ok(bytes) => {
                        uploaded.push(UploadedFile::detect(file.name(), bytes));
                    }
                    Err(e) => {
                        set_state.set(AppState::Error(format!("Failed to read file: {:?}", e)));
                        return;
                    }
                }
            }
            match run_analysis(uploaded).await {
                Ok(result) => set_state.set(AppState::Done(Arc::new(result))),
                Err(e) => set_state.set(AppState::Error(e.to_string())),
            }
        });
    };

    let on_input_change = move |ev: web_sys::Event| {
        let input: HtmlInputElement = ev.target().unwrap().dyn_into().unwrap();
        if let Some(file_list) = input.files() {
            let mut files = Vec::new();
            for i in 0..file_list.length() {
                if let Some(f) = file_list.get(i) {
                    files.push(f);
                }
            }
            on_files_selected(files);
        }
    };

    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            if let Some(file_list) = dt.files() {
                let mut files = Vec::new();
                for i in 0..file_list.length() {
                    if let Some(f) = file_list.get(i) {
                        files.push(f);
                    }
                }
                on_files_selected(files);
            }
        }
    };

    let on_dragover = |ev: web_sys::DragEvent| ev.prevent_default();

    view! {
        <div class="upload-wrapper">
            <div class="upload-zone" on:drop=on_drop on:dragover=on_dragover>
                {if is_loading {
                    view! {
                        <div class="spinner-box">
                            <div class="spinner"></div>
                            <p>"Analysing your data…"</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="upload-prompt">
                            <div class="upload-icon">"📊"</div>
                            <p class="upload-title">"Drop files here"</p>
                            <p class="upload-subtitle">".xlsx  ·  .xls  ·  .csv"</p>
                            <label class="btn btn-primary upload-btn">
                                "Browse files"
                                <input
                                    type="file"
                                    accept=".xlsx,.xls,.csv"
                                    multiple=true
                                    style="display:none"
                                    on:change=on_input_change
                                />
                            </label>
                            <p class="upload-note">
                                "Files are processed locally — nothing is uploaded to a server."
                            </p>
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}
