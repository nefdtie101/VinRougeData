use std::path::{Path, PathBuf};
use std::sync::Arc;

use vinrouge::dsl::{self, parse_value, InMemoryDataSource, StatementResult};
use vinrouge::projects::{self, DslScript};

use crate::session_reader::SessionReader;
use crate::state::LoadedProject;

pub struct ScriptRun {
    pub script_id: String,
    pub label: String,
    pub results: Vec<StatementResult>,
}

/// Scan `data_dir` for `.vrd` files, extract each one, and return loaded projects.
pub fn scan_data_dir(data_dir: &Path) -> anyhow::Result<Vec<LoadedProject>> {
    let mut found = Vec::new();

    for entry in std::fs::read_dir(data_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vrd") {
            continue;
        }
        match projects::vrd::import_project_vrd(&path, data_dir) {
            Ok(project) => {
                let project_dir = PathBuf::from(&project.path);
                tracing::info!("Loaded project: {} ({})", project.name, project.id);
                found.push(LoadedProject { project, project_dir });
            }
            Err(e) => tracing::warn!("Skipping {}: {e}", path.display()),
        }
    }

    Ok(found)
}

/// Build an in-memory datasource from all session imports in the project directory.
pub fn build_datasource(project_dir: &Path) -> Result<InMemoryDataSource, String> {
    let conn = projects::db::open_project(project_dir).map_err(|e| e.to_string())?;
    let reader = SessionReader::new(&conn);
    let imports = reader.list_imports()?;
    let mut datasource = InMemoryDataSource::new();

    for imp in &imports {
        let rows_raw = reader.get_rows(&imp.id)?;
        let table_name = table_name_from_source(&imp.source_name);
        let rows: Vec<vinrouge::dsl::Row> = rows_raw
            .into_iter()
            .map(|map| map.into_iter().map(|(k, v)| (k, parse_value(v))).collect())
            .collect();
        datasource.insert_table(table_name, rows);
    }

    Ok(datasource)
}

/// Parse and run a DSL script text, returning the raw StatementResults.
pub fn run_script(
    script_text: &str,
    datasource: Arc<InMemoryDataSource>,
) -> Result<Vec<StatementResult>, String> {
    let statements =
        dsl::parse(script_text).map_err(|e| format!("DSL parse error: {}", e.message))?;
    Ok(dsl::run_script(&statements, datasource.as_ref()))
}

/// Run every script from the given list against the datasource.
/// Scripts come from the .vrd file loaded at startup — no disk reads here.
pub fn run_all_scripts(
    scripts: &[DslScript],
    datasource: Arc<InMemoryDataSource>,
) -> Vec<ScriptRun> {
    scripts.iter().map(|script| {
        let results = run_script(&script.script_text, datasource.clone())
            .unwrap_or_else(|e| vec![StatementResult::Error(e)]);
        ScriptRun { script_id: script.id.clone(), label: script.label.clone(), results }
    }).collect()
}

fn table_name_from_source(source_name: &str) -> String {
    let base = if let Some(start) = source_name.find('[') {
        let sheet = &source_name[start + 1..];
        sheet.trim_end_matches(']').trim()
    } else {
        Path::new(source_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(source_name)
    };
    base.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
