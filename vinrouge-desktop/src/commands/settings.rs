use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};
use vinrouge::settings::AppSettings;

/// Return the current application settings.
#[tauri::command]
pub fn get_settings() -> Result<AppSettings, String> {
    Ok(AppSettings::load())
}

/// Persist application settings and notify listeners.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    settings.save()?;
    let _ = app.emit("settings-saved", ());
    Ok(())
}

/// Close the settings window.
#[tauri::command]
pub fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a native settings window (served from the app bundle).
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // If a settings window already exists, close it first
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.close();
    }

    let window = WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("Settings")
    .inner_size(480.0, 520.0)
    .resizable(false)
    .maximizable(false)
    .build()
    .map_err(|e| format!("Failed to create window: {e}"))?;

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = app_handle.emit("settings-closed", ());
        }
    });

    Ok(())
}
