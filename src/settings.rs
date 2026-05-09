use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// AI provider selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Application settings persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub provider: AiProvider,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default)]
    pub ollama_models_dir: Option<String>,
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
        Self {
            provider: AiProvider::default(),
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            ollama_models_dir: None,
            terminal_command: String::new(),
        }
    }
}

impl AppSettings {
    /// Path to the settings TOML file (`~/.config/vinrouge/settings.toml`).
    pub fn settings_path() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok();
        if let Some(home) = home {
            PathBuf::from(home)
                .join(".config")
                .join("vinrouge")
                .join("settings.toml")
        } else {
            PathBuf::from("vinrouge-settings.toml")
        }
    }

    /// Load settings from disk, returning defaults if the file is missing or corrupt.
    pub fn load() -> Self {
        let path = Self::settings_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save settings to disk, creating parent directories if needed.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        std::fs::write(&path, content).map_err(|e| format!("Failed to write settings: {e}"))
    }
}
