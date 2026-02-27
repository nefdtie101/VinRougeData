use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sources: Vec<SourceConfig>,
    pub export: ExportConfig,
    pub analysis: AnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SourceConfig {
    Mssql {
        connection_string: String,
        name: Option<String>,
    },
    Csv {
        path: String,
        delimiter: Option<char>,
        has_header: Option<bool>,
    },
    Excel {
        path: String,
        sheet: Option<String>,
        has_header: Option<bool>,
    },
    Flatfile {
        path: String,
        delimiter: Option<char>,
        column_widths: Option<Vec<usize>>,
        column_names: Option<Vec<String>>,
        has_header: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_format")]
    pub format: String, // "json", "markdown", "console"

    #[serde(default)]
    pub output_path: Option<String>,

    #[serde(default = "default_true")]
    pub pretty: bool,

    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default = "default_true")]
    pub detect_relationships: bool,

    #[serde(default = "default_true")]
    pub detect_workflows: bool,

    #[serde(default = "default_confidence")]
    pub min_confidence: u8,
}

fn default_format() -> String {
    "console".to_string()
}

fn default_true() -> bool {
    true
}

fn default_confidence() -> u8 {
    50
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            output_path: None,
            pretty: true,
            verbose: false,
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            detect_relationships: true,
            detect_workflows: true,
            min_confidence: 50,
        }
    }
}

impl AppConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read config file")?;

        // Support both JSON and TOML
        if path.as_ref().extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content).context("Failed to parse TOML config")
        } else {
            serde_json::from_str(&content).context("Failed to parse JSON config")
        }
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = if path.as_ref().extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::to_string_pretty(self).context("Failed to serialize to TOML")?
        } else {
            serde_json::to_string_pretty(self).context("Failed to serialize to JSON")?
        };

        std::fs::write(path, content).context("Failed to write config file")?;
        Ok(())
    }
}
