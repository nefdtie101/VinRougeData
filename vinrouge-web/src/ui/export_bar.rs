use leptos::prelude::*;
use std::sync::Arc;
use vinrouge::export::{AnalysisResult, ExcelExporter, Exporter, JsonExporter, MarkdownExporter};
use wasm_bindgen::JsCast;

fn trigger_download(filename: &str, bytes: &[u8], mime: &str) {
    let uint8 = js_sys::Uint8Array::from(bytes);
    let array = js_sys::Array::new();
    array.push(&uint8.buffer());

    let mut opts = web_sys::BlobPropertyBag::new();
    opts.type_(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&array, &opts).unwrap();
    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let a: web_sys::HtmlAnchorElement = document.create_element("a").unwrap().dyn_into().unwrap();
    a.set_href(&url);
    a.set_download(filename);
    document.body().unwrap().append_child(&a).unwrap();
    a.click();
    document.body().unwrap().remove_child(&a).unwrap();
    web_sys::Url::revoke_object_url(&url).unwrap();
}

#[component]
pub fn ExportBar(result: Arc<AnalysisResult>) -> impl IntoView {
    let result_json = result.clone();
    let result_md = result.clone();
    let result_xlsx = result.clone();

    let export_json = move |_| {
        let exporter = JsonExporter::new(true);
        if let Ok(json) = exporter.export(&result_json) {
            trigger_download(
                "vinrouge-analysis.json",
                json.as_bytes(),
                "application/json",
            );
        }
    };

    let export_md = move |_| {
        let exporter = MarkdownExporter;
        if let Ok(md) = exporter.export(&result_md) {
            trigger_download("vinrouge-analysis.md", md.as_bytes(), "text/markdown");
        }
    };

    let export_xlsx = move |_| {
        let exporter = ExcelExporter::new(String::new());
        if let Ok(bytes) = exporter.export_to_bytes(&result_xlsx) {
            trigger_download(
                "vinrouge-analysis.xlsx",
                &bytes,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            );
        }
    };

    view! {
        <div class="export-bar">
            <span class="export-label">"Export:"</span>
            <button class="btn btn-export" on:click=export_json>"JSON"</button>
            <button class="btn btn-export" on:click=export_md>"Markdown"</button>
            <button class="btn btn-export" on:click=export_xlsx>"Excel"</button>
        </div>
    }
}
