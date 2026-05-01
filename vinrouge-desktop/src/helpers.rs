use crate::state::AnalysisOutput;
use std::path::PathBuf;
use vinrouge::analysis::{RelationshipDetector, WorkflowDetector};
use vinrouge::dsl::{self, parse_value, InMemoryDataSource, StatementResult};
use vinrouge::schema::Table;
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};

// ── Windows: suppress the console window for child processes ─────────────────

#[cfg(target_os = "windows")]
pub trait NoConsole {
    fn no_console(&mut self) -> &mut Self;
}

#[cfg(target_os = "windows")]
impl NoConsole for std::process::Command {
    fn no_console(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW)
    }
}

// ── Core analysis (runs in its own current-thread Tokio runtime) ──────────────
//
// DataSource::extract_schema() uses `async_trait(?Send)` for WASM compat, so
// its Future is !Send.  We must run it on a current_thread runtime; spinning
// one up on a fresh OS thread is the cleanest way to do that.

pub async fn run_analysis(path: String) -> Result<AnalysisOutput, String> {
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let tables: Vec<Table> = if ext == "csv" {
        CsvSource::new(path.clone())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else if ext == "xlsx" || ext == "xls" {
        ExcelSource::new(path.clone())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else {
        return Err(format!("Unsupported file type: .{ext}"));
    };

    let relationships = RelationshipDetector::new(tables.clone()).detect_relationships();
    let workflows =
        WorkflowDetector::new(tables.clone(), relationships.clone()).detect_workflows();

    Ok(AnalysisOutput {
        tables,
        relationships,
        workflows,
    })
}

// ── DSL helpers ───────────────────────────────────────────────────────────────

/// Derive a DSL table name from a file's source_name (e.g. "Sales Data.xlsx" → "sales_data").
pub fn table_name_from_source(source_name: &str) -> String {
    // If the source name contains a sheet suffix like "file.xlsx [SheetName]",
    // use the sheet name as the table name so each sheet gets a distinct identifier.
    let base = if let Some(start) = source_name.find('[') {
        let sheet = &source_name[start + 1..];
        let sheet = sheet.trim_end_matches(']').trim();
        sheet
    } else {
        // No sheet suffix — use the file stem (name without extension)
        std::path::Path::new(source_name)
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

pub fn run_dsl_script_blocking(
    script_id: String,
    project_dir: PathBuf,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;

    // Load the script text
    let script: vinrouge::projects::DslScript = {
        let scripts = vinrouge::projects::list_dsl_scripts(&project_dir)?;
        scripts
            .into_iter()
            .find(|s| s.id == script_id)
            .ok_or_else(|| format!("Script {script_id} not found"))?
    };

    // Load all session imports and build an InMemoryDataSource
    let db = crate::session_db::SessionDb::new(&conn);
    let imports = db.list_imports()?;
    let mut datasource = InMemoryDataSource::new();

    for imp in &imports {
        let raw_rows = db.get_rows(&imp.id)?;
        let table_name = table_name_from_source(&imp.source_name);
        let rows: Vec<vinrouge::dsl::Row> = raw_rows
            .into_iter()
            .map(|map| {
                map.into_iter()
                    .map(|(k, v)| (k, parse_value(v)))
                    .collect()
            })
            .collect();
        datasource.insert_table(table_name, rows);
    }

    // Parse and run the script
    let statements = dsl::parse(&script.script_text)
        .map_err(|e| format!("DSL parse error: {}", e.message))?;
    let raw_results = dsl::run_script(&statements, &datasource);

    // Serialise results to JSON
    let json_results: Vec<serde_json::Value> = raw_results
        .iter()
        .enumerate()
        .map(|(i, r)| match r {
            StatementResult::Assert(a) => serde_json::json!({
                "kind": "assert",
                "index": i,
                "label": a.label,
                "passed": a.passed,
                "lhs_value": a.lhs_value,
                "rhs_value": a.rhs_value,
                "op": a.op,
            }),
            StatementResult::Sample(s) => serde_json::json!({
                "kind": "sample",
                "index": i,
                "method": format!("{:?}", s.method),
                "population_size": s.population_size,
                "selected_count": s.selected.len(),
                "selected_indices": s.selected,
            }),
            StatementResult::Relation { from, to } => serde_json::json!({
                "kind": "relation",
                "index": i,
                "from": from,
                "to": to,
            }),
            StatementResult::Value(v) => serde_json::json!({
                "kind": "value",
                "index": i,
                "value": v,
            }),
            StatementResult::Error(e) => serde_json::json!({
                "kind": "error",
                "index": i,
                "error": e,
            }),
        })
        .collect();

    // Count outcomes
    let passed = json_results
        .iter()
        .filter(|r| r["kind"] == "assert" && r["passed"] == true)
        .count() as i64;
    let failed = json_results
        .iter()
        .filter(|r| r["kind"] == "assert" && r["passed"] == false)
        .count() as i64;
    let errors = json_results
        .iter()
        .filter(|r| r["kind"] == "error")
        .count() as i64;

    let result_json = serde_json::to_string(&json_results)
        .map_err(|e| format!("Serialise error: {e}"))?;
    vinrouge::projects::save_test_result(
        &project_dir,
        &script_id,
        &result_json,
        passed,
        failed,
        errors,
    )?;

    Ok(json_results)
}
