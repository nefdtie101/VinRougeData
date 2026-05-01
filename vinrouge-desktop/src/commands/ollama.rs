use crate::state::OllamaState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// Progress payload emitted to the frontend during a model pull.
#[derive(Serialize, Clone)]
struct PullProgress {
    percent: u8,
    status: String,
    done: bool,
}

/// Start the bundled Ollama server. Safe to call multiple times — does nothing
/// if it is already running. Returns the resolved binary path on success.
#[tauri::command]
pub fn start_ollama(state: State<OllamaState>) -> Result<String, String> {
    let mut guard = state.0.lock().unwrap();

    // Already running (process we spawned)?
    if let Some(child) = guard.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return Ok("already running".to_string());
        }
    }

    // Already running externally (e.g. Windows service or user-started instance)?
    if vinrouge::ollama::port_in_use(11434) {
        return Ok("already running".to_string());
    }

    let binary = vinrouge::ollama::find_binary().map_err(|e| e.to_string())?;
    let binary_str = binary.to_string_lossy().to_string();

    let mut cmd = std::process::Command::new(&binary);
    cmd.arg("serve");
    #[cfg(target_os = "windows")]
    crate::helpers::NoConsole::no_console(&mut cmd);

    // Read user override from shared settings file, fall back to DEFAULT_MODELS_DIR
    let saved_dir: Option<String> = (|| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        let path = std::path::PathBuf::from(home)
            .join(".config")
            .join("vinrouge")
            .join("tui.toml");
        let content = std::fs::read_to_string(path).ok()?;
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("ollama_models_dir = ") {
                let dir = val.trim().trim_matches('"').to_string();
                if !dir.is_empty() {
                    return Some(dir);
                }
            }
        }
        None
    })();

    if let Some(dir) = vinrouge::ollama::resolve_models_dir(saved_dir.as_deref()) {
        cmd.env("OLLAMA_MODELS", dir);
    }
    // Allow the Tauri WebView origin (tauri://localhost / http://tauri.localhost)
    // to call Ollama's HTTP API without CORS rejections.
    cmd.env("OLLAMA_ORIGINS", "*");

    // On Windows, prepend the bundled CUDA runner directory to PATH so that
    // when ggml-cuda.dll is loaded by Ollama it can find its dependencies
    // (cublas64_12.dll, cudart64_12.dll, etc.) via the normal DLL search.
    #[cfg(target_os = "windows")]
    if let Some(binary_dir) = binary.parent() {
        let cuda_dir = binary_dir.join("lib").join("ollama").join("cuda_v12");
        if cuda_dir.exists() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{};{}", cuda_dir.display(), current_path));
        }
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to start Ollama: {e}"))?;
    *guard = Some(child);
    Ok(binary_str)
}

/// Stop the Ollama server that was started by this app.
#[tauri::command]
pub fn stop_ollama(state: State<OllamaState>) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        child.kill().map_err(|e| e.to_string())?;
    }
    *guard = None;
    Ok(())
}

/// Returns `true` if the Ollama process we spawned is still alive.
#[tauri::command]
pub fn ollama_running(state: State<OllamaState>) -> bool {
    let mut guard = state.0.lock().unwrap();
    match guard.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

/// Check whether the `mistral` model is already available in the local Ollama
/// instance.  Waits up to 10 s for the server to become reachable before
/// checking, so it is safe to call right after `start_ollama`.
#[tauri::command]
pub async fn check_model() -> Result<bool, String> {
    let client = reqwest::Client::new();

    // Poll until the server is up (max 10 s)
    for _ in 0..10 {
        if client
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let resp = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Ollama not reachable: {e}"))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let has_mistral = body["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|m| m["name"].as_str().unwrap_or("").starts_with(vinrouge::ollama::DEFAULT_MODEL))
        })
        .unwrap_or(false);

    Ok(has_mistral)
}

/// Pull the default model from the Ollama registry.  Streams the response
/// line-by-line and emits `model-pull-progress` events so the frontend can
/// show a live percentage bar.  Returns once the pull is complete.
#[tauri::command]
pub async fn pull_model(app: AppHandle) -> Result<(), String> {
    let client = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .map_err(|e| e.to_string())?;

    let mut resp = client
        .post("http://localhost:11434/api/pull")
        .json(&serde_json::json!({"name": vinrouge::ollama::DEFAULT_MODEL, "stream": true}))
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Pull failed ({status}): {body}"));
    }

    let mut buf = String::new();

    loop {
        match resp.chunk().await.map_err(|e| e.to_string())? {
            None => break,
            Some(bytes) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].to_string();
                    buf.drain(..=pos);

                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        // Ollama surfaces errors inside the stream
                        if let Some(err) = val["error"].as_str() {
                            let _ = app.emit(
                                "model-pull-progress",
                                PullProgress {
                                    percent: 0,
                                    status: format!("Error: {err}"),
                                    done: true,
                                },
                            );
                            return Err(format!("Model pull error: {err}"));
                        }

                        let status = val["status"].as_str().unwrap_or("").to_string();
                        let done = status == "success";

                        let percent = if done {
                            100
                        } else if let (Some(total), Some(completed)) =
                            (val["total"].as_u64(), val["completed"].as_u64())
                        {
                            if total > 0 {
                                ((completed * 99) / total) as u8 // cap at 99 until done
                            } else {
                                0
                            }
                        } else {
                            0
                        };

                        let _ = app.emit(
                            "model-pull-progress",
                            PullProgress { percent, status, done },
                        );

                        if done {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
