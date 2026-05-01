use crate::helpers::table_name_from_source;
use crate::session_db::SessionDb;
use crate::state::ProjectsState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSchema {
    pub import_id: String,
    pub source_type: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub row_count: usize,
    /// (original_col_name, pbc_col_name) pairs for non-master imports; empty for master.
    pub col_map: Vec<(String, String)>,
}

/// Parse a project CSV/Excel file and write mapped rows into the session store.
#[tauri::command]
pub async fn import_data_file(
    file_id: String,
    mappings: Vec<(String, String)>,
    sheet: Option<String>,
    state: State<'_, ProjectsState>,
) -> Result<String, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || {
        let path = vinrouge::projects::get_file_path(&project_dir, &file_id)?;
        let bytes = std::fs::read(&path).map_err(|e| format!("Read error: {e}"))?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
        let db = SessionDb::new(&conn);

        match ext.as_str() {
            "csv" => db.import_csv(Some(&file_id), &name, bytes, &mappings),
            "xlsx" | "xls" => {
                db.import_excel(Some(&file_id), &name, bytes, &mappings, sheet.as_deref())
                    .map(|ids| ids.into_iter().next().unwrap_or_default())
            }
            _ => Err(format!("Unsupported file type: .{ext}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn save_column_mappings(
    file_id: String,
    mappings: Vec<(String, String)>,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let json = serde_json::to_string(&mappings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO file_column_mappings (file_id, mappings_json, updated_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![file_id, json, now],
    )
    .map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_column_mappings(
    file_id: String,
    state: State<ProjectsState>,
) -> Result<Vec<(String, String)>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
    let json: Option<String> = conn
        .query_row(
            "SELECT mappings_json FROM file_column_mappings WHERE file_id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn list_session_imports(
    state: State<ProjectsState>,
) -> Result<Vec<crate::session_db::SessionImport>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
    SessionDb::new(&conn).list_imports()
}

#[tauri::command]
pub fn get_session_rows(
    import_id: String,
    state: State<ProjectsState>,
) -> Result<Vec<std::collections::HashMap<String, String>>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
    SessionDb::new(&conn).get_rows(&import_id)
}

#[tauri::command]
pub async fn get_session_rows_paged(
    import_id: String,
    offset: usize,
    limit: usize,
    state: State<'_, ProjectsState>,
) -> Result<Vec<std::collections::HashMap<String, String>>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || {
        let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
        SessionDb::new(&conn).get_rows_paged(&import_id, offset, limit)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn delete_session_import(
    import_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
    SessionDb::new(&conn).delete_import(&import_id)
}

/// Return one SessionSchema per import.
#[tauri::command]
pub async fn get_session_schemas(
    state: State<'_, ProjectsState>,
) -> Result<Vec<SessionSchema>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || {
        let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
        let db = SessionDb::new(&conn);
        let imports = db.list_imports()?;
        imports
            .into_iter()
            .map(|imp| {
                let table_name = table_name_from_source(&imp.source_name);
                let columns: Vec<String> = db.get_import_columns(&imp.id).unwrap_or_default();
                let col_map: Vec<(String, String)> = if imp.mappings.is_empty() {
                    vec![]
                } else {
                    imp.mappings
                        .iter()
                        .filter(|(src, _)| !src.trim().is_empty())
                        .map(|(src, tgt)| {
                            let pbc = if tgt.is_empty() { src.clone() } else { tgt.clone() };
                            (src.clone(), pbc)
                        })
                        .collect()
                };
                Ok(SessionSchema {
                    import_id: imp.id,
                    source_type: imp.source_type,
                    table_name,
                    columns,
                    row_count: imp.row_count,
                    col_map,
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Detect likely join keys across all non-master session imports.
#[tauri::command]
pub async fn detect_data_relationships(
    state: State<'_, ProjectsState>,
) -> Result<Vec<vinrouge::RelCandidate>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || {
        let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
        let db = SessionDb::new(&conn);

        // Step 1 — load imports and build schema tables
        let all_imports = db.list_imports()?;
        let imports: Vec<&crate::session_db::SessionImport> = all_imports.iter().collect();

        if imports.len() < 2 {
            return Ok(vec![]);
        }

        let mut tables = Vec::new();
        let mut table_to_import = std::collections::HashMap::new();

        for imp in &imports {
            match db.build_schema_table(imp) {
                Ok(t) => {
                    table_to_import.insert(t.full_name.clone(), imp.id.clone());
                    tables.push(t);
                }
                Err(e) => eprintln!("[detect_relationships] skipping {}: {e}", imp.source_name),
            }
        }

        if tables.len() < 2 {
            return Ok(vec![]);
        }

        // Step 2 — name/pattern detection
        let relationships = vinrouge::analysis::RelationshipDetector::new(tables).detect_relationships();

        // Step 3 — fetch value samples for every detected candidate
        let mut samples: std::collections::HashMap<(String, String), Vec<String>> =
            std::collections::HashMap::new();

        for rel in &relationships {
            let Some(lid) = table_to_import.get(&rel.from_table) else { continue };
            let Some(rid) = table_to_import.get(&rel.to_table) else { continue };

            let lk = (lid.clone(), rel.from_column.clone());
            if !samples.contains_key(&lk) {
                samples.insert(lk, db.sample_column(lid, &rel.from_column, 200)?);
            }
            let rk = (rid.clone(), rel.to_column.clone());
            if !samples.contains_key(&rk) {
                samples.insert(rk, db.sample_column(rid, &rel.to_column, 200)?);
            }
        }

        // Step 4 — score
        Ok(vinrouge::analysis::RelationshipScorer::score(
            &relationships,
            &table_to_import,
            &samples,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return value-frequency distribution for a single column in a session table.
#[tauri::command]
pub async fn get_column_distribution(
    table_col: String,
    state: State<'_, ProjectsState>,
) -> Result<Vec<serde_json::Value>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    tokio::task::spawn_blocking(move || {
        let conn = vinrouge::projects::db::open_project(&project_dir).map_err(|e| e.to_string())?;
        let db = SessionDb::new(&conn);

        let (table_part, col_part) = table_col
            .split_once('.')
            .ok_or_else(|| format!("expected table.column, got: {table_col}"))?;

        let imports = db.list_imports()?;
        let import = imports
            .iter()
            .find(|imp| table_name_from_source(&imp.source_name).eq_ignore_ascii_case(table_part))
            .ok_or_else(|| format!("table '{table_part}' not found in session imports"))?;

        let rows = db.get_rows(&import.id)?;

        let mut counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for row in &rows {
            let val = row
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(col_part))
                .map(|(_, v)| v.as_str())
                .unwrap_or("")
                .to_string();
            *counts.entry(val).or_insert(0) += 1;
        }

        let mut entries: Vec<(String, usize)> = counts.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        entries.truncate(50);

        Ok(entries
            .into_iter()
            .map(|(value, count)| serde_json::json!({ "value": value, "count": count }))
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
