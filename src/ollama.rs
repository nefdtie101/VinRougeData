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
    let expanded = expand_home(raw);
    // If the path still starts with '~' the expansion failed (no HOME/USERPROFILE).
    // Return None so the caller does NOT set OLLAMA_MODELS and Ollama uses its
    // own platform default instead of crashing on a literal '~' path.
    if expanded.starts_with('~') {
        return None;
    }
    Some(expanded)
}

#[cfg(not(target_arch = "wasm32"))]
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        // HOME on Unix/macOS, USERPROFILE on Windows
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok();
        if let Some(home) = home {
            return std::path::PathBuf::from(home)
                .join(rest)
                .to_string_lossy()
                .into_owned();
        }
        // Can't expand — return None-equivalent by returning the raw path so
        // the caller can decide not to set OLLAMA_MODELS at all.
        return path.to_string();
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
            // `where` on Windows can return multiple lines; take only the first.
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(path) = stdout.lines().find(|l| !l.trim().is_empty()) {
                return Ok(std::path::PathBuf::from(path.trim()));
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

/// Sends `prompt` to Ollama using structured output (JSON Schema as the format).
/// The model is constrained via token sampling to match the schema exactly.
/// Non-wasm stub — not used on native.
#[cfg(not(target_arch = "wasm32"))]
pub async fn ask_ollama_structured(
    _base_url: &str,
    _model: &str,
    _prompt: &str,
    _schema: serde_json::Value,
) -> Result<String, String> {
    Err("ask_ollama_structured is only available on wasm32".to_string())
}

/// Structured output — wasm32 implementation.
#[cfg(target_arch = "wasm32")]
pub async fn ask_ollama_structured(
    base_url: &str,
    model: &str,
    prompt: &str,
    schema: serde_json::Value,
) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    let body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a JSON generation assistant. \
                            Respond with a valid JSON object that matches the provided schema. \
                            Do not add explanations, markdown, or code fences — output only the JSON object."
            },
            {"role": "user", "content": prompt}
        ],
        "stream": false,
        "format": schema
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

/// Extract the first JSON object or array from a free-text response.
/// Handles ```json ... ``` fences, plain ``` ... ``` fences, and bare { / [ .
#[cfg(target_arch = "wasm32")]
fn extract_json_from_text(text: &str) -> Option<String> {
    // 1. ```json ... ```
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            let candidate = rest[..end].trim();
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }
    // 2. ``` ... ``` (no language tag)
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let candidate = rest[..end].trim();
            if candidate.starts_with('{') || candidate.starts_with('[') {
                if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    // 3. Bare { ... } — find the first '{' and the matching '}'
    if let Some(obj_start) = text.find('{') {
        let slice = &text[obj_start..];
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;
        let mut end_idx = None;
        for (i, c) in slice.char_indices() {
            if escape { escape = false; continue; }
            if c == '\\' && in_string { escape = true; continue; }
            if c == '"' { in_string = !in_string; continue; }
            if in_string { continue; }
            match c {
                '{' => depth += 1,
                '}' => { depth -= 1; if depth == 0 { end_idx = Some(i); break; } }
                _ => {}
            }
        }
        if let Some(end) = end_idx {
            let candidate = &slice[..=end];
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }
    // 4. Bare [ ... ]
    if let Some(arr_start) = text.find('[') {
        let slice = &text[arr_start..];
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;
        let mut end_idx = None;
        for (i, c) in slice.char_indices() {
            if escape { escape = false; continue; }
            if c == '\\' && in_string { escape = true; continue; }
            if c == '"' { in_string = !in_string; continue; }
            if in_string { continue; }
            match c {
                '[' => depth += 1,
                ']' => { depth -= 1; if depth == 0 { end_idx = Some(i); break; } }
                _ => {}
            }
        }
        if let Some(end) = end_idx {
            let candidate = &slice[..=end];
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
pub async fn ask_ollama_json(base_url: &str, model: &str, prompt: &str) -> Result<String, String> {
    use gloo_net::http::Request;
    use serde_json::json;

    // Do NOT use "format":"json" — it forces valid JSON but small models like
    // mistral then choose their own schema. Instead, ask the model to wrap its
    // JSON in a ```json fence and extract it from the free-text reply.
    let body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a JSON generation assistant. \
                            When asked to produce JSON, output it inside a ```json code fence \
                            and nothing else. Do not add explanations before or after the fence."
            },
            {"role": "user", "content": prompt}
        ],
        "stream": false
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

    let content = val["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("Unexpected response shape: {val}"))?;

    // If the model output is already valid JSON (some models still do this),
    // return it directly; otherwise extract from the code fence / bare braces.
    if serde_json::from_str::<serde_json::Value>(content).is_ok() {
        return Ok(content.to_string());
    }

    extract_json_from_text(content)
        .ok_or_else(|| {
            let preview: String = content.chars().take(300).collect();
            format!("Could not find JSON in model response. Preview: {preview}")
        })
}
