use tauri::{AppHandle, Emitter, WebviewWindowBuilder};

/// Open a new window to display DSL results with ECharts-rendered charts.
///
/// Uses the shared HTML page renderer from the core `vinrouge` crate.
#[tauri::command]
pub async fn open_dsl_results_window(
    app: AppHandle,
    results: Vec<serde_json::Value>,
) -> Result<(), String> {
    let html = vinrouge::dsl::html::render_html_page(&results);

    // Write HTML to a temporary file
    use std::io::Write;
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vinrouge_dsl_results.html");

    std::fs::File::create(&file_path)
        .and_then(|mut f| f.write_all(html.as_bytes()))
        .map_err(|e| format!("Failed to write temp file: {e}"))?;

    // Build a well-formed file:// URL. On Windows, display() uses backslashes and the
    // path starts with a drive letter, so we need three slashes and forward slashes.
    #[cfg(windows)]
    let file_url = format!("file:/// {}", file_path.to_string_lossy().replace('\\', "/"));
    #[cfg(not(windows))]
    let file_url = format!("file://{}", file_path.display());
    let window = WebviewWindowBuilder::new(
        &app,
        "dsl-results",
        tauri::WebviewUrl::External(file_url.parse().unwrap()),
    )
    .title("DSL Results")
    .inner_size(1200.0, 800.0)
    .build()
    .map_err(|e| format!("Failed to create window: {e}"))?;

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = app_handle.emit("dsl-results-closed", ());
        }
    });

    Ok(())
}
