/// Ollama helpers — re-exported from the main vinrouge library.
pub use vinrouge::ollama::{
    ask_ollama_json, ask_ollama_structured, ask_ollama_wasm,
    DEFAULT_MODEL as OLLAMA_DEFAULT_MODEL, DEFAULT_URL as OLLAMA_DEFAULT_URL,
};

use crate::types::AnalysisResult;

// ── Analysis summary for Ollama context ───────────────────────────────────────

pub fn build_web_summary(result: &AnalysisResult) -> String {
    let mut s = String::new();

    s.push_str(&format!("Tables ({}):\n", result.tables.len()));
    for t in &result.tables {
        s.push_str(&format!(
            "  - {} ({} columns, ~{} rows)\n",
            t.name,
            t.columns.len(),
            t.row_count.unwrap_or(0)
        ));
        for c in &t.columns {
            s.push_str(&format!("      {}: {:?}\n", c.name, c.data_type));
        }
    }

    if !result.relationships.is_empty() {
        s.push_str(&format!(
            "\nRelationships ({}):\n",
            result.relationships.len()
        ));
        for r in &result.relationships {
            s.push_str(&format!(
                "  - {}.{} -> {}.{}\n",
                r.from_table, r.from_column, r.to_table, r.to_column
            ));
        }
    }

    if !result.workflows.is_empty() {
        s.push_str(&format!("\nWorkflows ({}):\n", result.workflows.len()));
        for w in &result.workflows {
            s.push_str(&format!("  - {:?}: {}\n", w.workflow_type, w.description));
        }
    }

    s
}
