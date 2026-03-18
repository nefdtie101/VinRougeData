#[cfg(not(target_arch = "wasm32"))]
use anyhow::{anyhow, Result};
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};

// =============================================================================
// OLLAMA CONFIGURATION
// =============================================================================
//
// DEFAULT_MODEL — the model name exactly as it appears in `ollama list`.
//   Examples: "llama3.2"  "llama3.1:8b"  "mistral"  "phi3"  "gemma2:9b"
//
pub const DEFAULT_MODEL: &str = "mistral";
//
// DEFAULT_MODELS_DIR — where your model files live on disk.
//   Set to Some("~/.ollama/models") to point at the standard macOS location,
//   or any other path where you keep your .gguf / Ollama manifest files.
//   Use None to let Ollama use its built-in default (~/.ollama/models).
//   The ~ prefix is expanded to your home directory at runtime.
//
pub const DEFAULT_MODELS_DIR: Option<&str> = Some("~/.ollama/models");
//
// DEFAULT_URL — where Ollama listens.
//   Leave this unless you changed the port or use a remote instance.
//
pub const DEFAULT_URL: &str = "http://localhost:11434";
//
// =============================================================================

// ── Wire types (native only) ──────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize, Deserialize)]
struct Message<'a> {
    role: &'a str,
    #[serde(borrow)]
    content: std::borrow::Cow<'a, str>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct ChatResponse {
    message: OwnedMessage,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct OwnedMessage {
    content: String,
}

// ── Client (native only) ──────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// A thin async client for the Ollama `/api/chat` endpoint.
pub struct OllamaClient {
    base_url: String,
    pub model: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl OllamaClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            model: model.into(),
        }
    }

    /// Send a single user message and return the assistant reply.
    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let req = ChatRequest {
            model: &self.model,
            messages: vec![Message {
                role: "user",
                content: std::borrow::Cow::Borrowed(prompt),
            }],
            stream: false,
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to reach Ollama at {url}: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama returned {status}: {body}"));
        }

        let chat: ChatResponse = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Ollama response: {e}"))?;

        Ok(chat.message.content)
    }

    /// List models available in this Ollama instance.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct ModelEntry {
            name: String,
        }
        #[derive(Deserialize)]
        struct ListResponse {
            models: Vec<ModelEntry>,
        }

        let url = format!("{}/api/tags", self.base_url);
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to reach Ollama at {url}: {e}"))?;

        let list: ListResponse = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse model list: {e}"))?;

        Ok(list.models.into_iter().map(|m| m.name).collect())
    }
}

// ── Models directory (native only) ────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// Resolve the models directory to pass as `OLLAMA_MODELS`.
///
/// Priority:
///   1. `override_dir` — value saved by the user via the TUI (F3)
///   2. `DEFAULT_MODELS_DIR` constant above
///   3. `None` — let Ollama use its built-in default
///
/// A leading `~/` is expanded to the current user's home directory.
pub fn resolve_models_dir(override_dir: Option<&str>) -> Option<String> {
    let raw = override_dir.or(DEFAULT_MODELS_DIR)?;
    Some(expand_home(raw))
}

#[cfg(not(target_arch = "wasm32"))]
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

// ── Port management (native only) ─────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// Extract the port number from a URL like `http://localhost:11434`.
pub fn port_from_url(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(11434)
}

#[cfg(not(target_arch = "wasm32"))]
/// Returns `true` if something is already listening on `port`.
pub fn port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[cfg(not(target_arch = "wasm32"))]
/// Ask the OS for a free TCP port by binding to port 0.
pub fn find_free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

#[cfg(not(target_arch = "wasm32"))]
/// Return the port to use: the preferred port if free, otherwise a new free one.
/// Also returns whether the port changed.
pub fn resolve_port(preferred: u16) -> (u16, bool) {
    if !port_in_use(preferred) {
        (preferred, false)
    } else if let Some(free) = find_free_port() {
        (free, true)
    } else {
        (preferred, false) // give up and try the original
    }
}

// ── Binary discovery (native only) ───────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// Locate the `ollama` binary. Search order:
/// 1. Next to the running executable (bundled distribution / Tauri sidecar)
/// 2. `which ollama` via PATH
/// 3. Common macOS / Linux install locations
pub fn find_binary() -> std::io::Result<std::path::PathBuf> {
    let bin_name = if cfg!(target_os = "windows") {
        "ollama.exe"
    } else {
        "ollama"
    };

    // 1. Bundled next to the running executable (TUI zip or Tauri sidecar)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Plain name (TUI zip)
            let candidate = dir.join(bin_name);
            if candidate.exists() {
                return Ok(candidate);
            }
            // Tauri sidecar naming: ollama-aarch64-apple-darwin
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if s.starts_with("ollama-") && !s.ends_with(".d") {
                    return Ok(entry.path());
                }
            }
        }
    }

    // 2. PATH lookup (works when launched from a terminal)
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    if let Ok(out) = std::process::Command::new(which_cmd).arg("ollama").output() {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(std::path::PathBuf::from(path));
            }
        }
    }

    // 3. Known install locations
    for path in [
        "/opt/homebrew/bin/ollama", // Apple Silicon Homebrew
        "/usr/local/bin/ollama",    // Intel Mac Homebrew / Linux
        "/usr/bin/ollama",
    ] {
        if std::path::Path::new(path).exists() {
            return Ok(std::path::PathBuf::from(path));
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "ollama binary not found — place it next to this executable or install from https://ollama.com",
    ))
}

// ── Context builder (native only) ─────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// Build a prompt that gives Ollama a summary of the analysis result as context,
/// then appends the user's question.
pub fn build_prompt(analysis_summary: &str, user_question: &str) -> String {
    format!(
        "You are a data analyst assistant. The user has analysed a dataset with the following \
         schema and findings:\n\n{analysis_summary}\n\nBased on this information, answer the \
         following question concisely:\n\n{user_question}"
    )
}

// ── WASM HTTP helpers ─────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub async fn ask_ollama_wasm(
    _base_url: &str,
    _model: &str,
    _context: &str,
    _question: &str,
) -> Result<String, String> {
    Err("ask_ollama_wasm is only available on wasm32".to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn ask_ollama_wasm(
    base_url: &str,
    model: &str,
    context: &str,
    question: &str,
) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    let prompt = if context.trim().is_empty() {
        format!("You are a data analyst assistant. Answer the following question concisely:\n\n{question}")
    } else {
        format!(
            "You are a data analyst assistant. The user analysed a dataset with the following \
             schema and findings:\n\n{context}\n\nBased on this, answer concisely:\n\n{question}"
        )
    };

    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let resp = Request::post(&url)
        .json(&body)
        .map_err(|e| format!("Failed to build request: {e}"))?
        .send()
        .await
        .map_err(|e| {
            format!(
                "Could not reach Ollama at {url}. \
             Is it running? Did you set OLLAMA_ORIGINS=*? Error: {e}"
            )
        })?;

    if !resp.ok() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama returned HTTP {status}: {text}"));
    }

    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected response shape: {val}"))
}

/// Sends `prompt` to Ollama with `"format":"json"` and returns the raw JSON string.
#[cfg(not(target_arch = "wasm32"))]
pub async fn ask_ollama_json(
    _base_url: &str,
    _model: &str,
    _prompt: &str,
) -> Result<String, String> {
    Err("ask_ollama_json is only available on wasm32".to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn ask_ollama_json(base_url: &str, model: &str, prompt: &str) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
        "format": "json"
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let resp = Request::post(&url)
        .json(&body)
        .map_err(|e| format!("Failed to build request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Could not reach Ollama at {url}: {e}"))?;

    if !resp.ok() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama returned HTTP {status}: {text}"));
    }

    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected response shape: {val}"))
}
