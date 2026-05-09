// ── Audit setup state (persisted to localStorage) ─────────────────────────────

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditSetupState {
    pub step: u8,
    pub standards: Vec<(String, bool)>,
    pub scope: Vec<String>,
    pub approved: bool,
}

pub fn ls_set(key: &str, val: &str) {
    if let Some(s) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = s.set_item(key, val);
    }
}

pub fn ls_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item(key).ok())
        .flatten()
}

// ── App settings (AI provider, API keys, etc.) ────────────────────────────────

const SETTINGS_KEY: &str = "vinrouge_settings_v1";

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    Ollama,
    Terminal,
}

impl Default for AiProvider {
    fn default() -> Self {
        AiProvider::Ollama
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub provider: AiProvider,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default)]
    pub terminal_command: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "mistral".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            provider: AiProvider::default(),
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            terminal_command: String::new(),
        }
    }
}

impl AppSettings {
    /// Load from localStorage (fast, synchronous).
    pub fn load() -> Self {
        ls_get(SETTINGS_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            ls_set(SETTINGS_KEY, &json);
        }
    }

    /// Load from Tauri backend (desktop app). Also updates localStorage cache.
    pub async fn load_from_tauri() -> Result<Self, String> {
        let s: Self = crate::ipc::tauri_invoke("get_settings").await?;
        if let Ok(json) = serde_json::to_string(&s) {
            ls_set(SETTINGS_KEY, &json);
        }
        Ok(s)
    }

    /// Save via Tauri backend (desktop app). Also updates localStorage cache.
    pub async fn save_to_tauri(&self) -> Result<(), String> {
        crate::ipc::tauri_invoke_args::<()>("save_settings", serde_json::json!({"settings": self}))
            .await?;
        self.save();
        Ok(())
    }
}

/// On desktop startup, sync settings from Tauri backend into localStorage.
pub async fn sync_settings_from_tauri() {
    if crate::ipc::is_tauri() {
        if let Ok(s) = AppSettings::load_from_tauri().await {
            if let Ok(json) = serde_json::to_string(&s) {
                ls_set(SETTINGS_KEY, &json);
            }
        }
    }
}
