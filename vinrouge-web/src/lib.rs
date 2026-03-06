mod file_upload;
mod analysis_bridge;
mod ui;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    // Route panics to browser console.error instead of silent crashes
    console_error_panic_hook::set_once();
    // Route tracing events to console.log / console.warn / console.error
    tracing_wasm::set_as_global_default();

    leptos::mount_to_body(ui::app::App);
}
