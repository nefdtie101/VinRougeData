//! SessionDb — per-project store for imported audit data.
//!
//! Schema (added to the per-project SQLite DB):
//!
//! `session_imports` — one row per import operation (CSV / Excel / SQL)
//! `session_rows`    — one row per data row, JSON-keyed by mapped PBC field name

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::runtime::Builder;
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionImport {
    pub id: String,
    pub file_id: Option<String>,
    pub source_type: String,
    pub source_name: String,
    pub row_count: usize,
    pub mappings: Vec<(String, String)>,
    pub imported_at: String,
}

// ── SessionDb ─────────────────────────────────────────────────────────────────

pub struct SessionDb<'a> {
    conn: &'a Connection,
}

impl<'a> SessionDb<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    /// Rename source-column keys to PBC field names using the mapping table.
    /// Columns with no mapping (empty target) are dropped.
    fn apply_mappings(
        row: &[String],
        headers: &[String],
        mappings: &[(String, String)],
    ) -> HashMap<String, String> {
        let lookup: HashMap<&str, &str> =
            mappings.iter().map(|(s, t)| (s.as_str(), t.as_str())).collect();
        let mut out = HashMap::new();
        for (i, val) in row.iter().enumerate() {
            if let Some(hdr) = headers.get(i) {
                if let Some(&target) = lookup.get(hdr.as_str()) {
                    if !target.is_empty() {
                        out.insert(target.to_string(), val.clone());
                    }
                }
            }
        }
        out
    }

    /// Write an import record and its rows into the DB.
    fn write_import(
        &self,
        file_id: Option<&str>,
        source_type: &str,
        source_name: &str,
        mappings: &[(String, String)],
        rows: Vec<HashMap<String, String>>,
    ) -> Result<String, String> {
        let import_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mappings_json =
            serde_json::to_string(mappings).unwrap_or_else(|_| "[]".to_string());
        let row_count = rows.len();

        self.conn
            .execute(
                "INSERT OR REPLACE INTO session_imports \
                 (id, file_id, source_type, source_name, row_count, mappings_json, imported_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    import_id,
                    file_id,
                    source_type,
                    source_name,
                    row_count as i64,
                    mappings_json,
                    now,
                ],
            )
            .map_err(|e| format!("DB insert session_import: {e}"))?;

        for (idx, row_map) in rows.iter().enumerate() {
            let row_id = uuid::Uuid::new_v4().to_string();
            let data_json =
                serde_json::to_string(row_map).unwrap_or_else(|_| "{}".to_string());
            self.conn
                .execute(
                    "INSERT INTO session_rows (id, import_id, row_index, data_json) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![row_id, import_id, idx as i64, data_json],
                )
                .map_err(|e| format!("DB insert session_row {idx}: {e}"))?;
        }

        Ok(import_id)
    }

    // ── CSV import ────────────────────────────────────────────────────────────

    pub fn import_csv(
        &self,
        file_id: Option<&str>,
        name: &str,
        bytes: Vec<u8>,
        mappings: &[(String, String)],
    ) -> Result<String, String> {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;

        let (headers, data) = rt.block_on(async {
            let mut src = CsvSource::from_bytes(bytes, name.to_string());
            let tables = src.extract_schema().await.map_err(|e| e.to_string())?;
            let headers: Vec<String> = tables
                .into_iter()
                .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                .collect();
            let data = src.read_data().await.map_err(|e| e.to_string())?;
            Ok::<_, String>((headers, data))
        })?;

        let rows: Vec<HashMap<String, String>> = data
            .iter()
            .map(|row| Self::apply_mappings(row, &headers, mappings))
            .filter(|m| !m.is_empty())
            .collect();

        self.write_import(file_id, "csv", name, mappings, rows)
    }

    // ── Excel import ──────────────────────────────────────────────────────────

    pub fn import_excel(
        &self,
        file_id: Option<&str>,
        name: &str,
        bytes: Vec<u8>,
        mappings: &[(String, String)],
        sheet: Option<&str>,
    ) -> Result<String, String> {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;

        let (headers, data) = rt.block_on(async {
            let src = ExcelSource::from_bytes(bytes, name.to_string());
            let mut src = match sheet {
                Some(sh) => src.with_sheet(sh.to_string()),
                None => src,
            };
            let tables = src.extract_schema().await.map_err(|e| e.to_string())?;
            let headers: Vec<String> = tables
                .into_iter()
                .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                .collect();
            let data = src.read_data().await.map_err(|e| e.to_string())?;
            Ok::<_, String>((headers, data))
        })?;

        let rows: Vec<HashMap<String, String>> = data
            .iter()
            .map(|row| Self::apply_mappings(row, &headers, mappings))
            .filter(|m| !m.is_empty())
            .collect();

        self.write_import(file_id, "excel", name, mappings, rows)
    }

    // ── SQL / pre-parsed import ───────────────────────────────────────────────
    //
    // The caller is responsible for running the query and fetching rows.
    // Keys in `raw_rows` are source column names; mappings rename them to PBC fields.

    pub fn import_rows(
        &self,
        source_name: &str,
        source_type: &str,
        file_id: Option<&str>,
        mappings: &[(String, String)],
        raw_rows: Vec<HashMap<String, String>>,
    ) -> Result<String, String> {
        let lookup: HashMap<&str, &str> =
            mappings.iter().map(|(s, t)| (s.as_str(), t.as_str())).collect();

        let rows: Vec<HashMap<String, String>> = raw_rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .filter_map(|(k, v)| {
                        let target = lookup.get(k.as_str()).copied().unwrap_or(k.as_str());
                        if target.is_empty() {
                            None
                        } else {
                            Some((target.to_string(), v))
                        }
                    })
                    .collect()
            })
            .filter(|m: &HashMap<String, String>| !m.is_empty())
            .collect();

        self.write_import(file_id, source_type, source_name, mappings, rows)
    }

    // ── Read back ─────────────────────────────────────────────────────────────

    pub fn list_imports(&self) -> Result<Vec<SessionImport>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_id, source_type, source_name, row_count, \
                 mappings_json, imported_at FROM session_imports ORDER BY imported_at DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        rows.into_iter()
            .map(
                |(id, file_id, source_type, source_name, row_count, mappings_json, imported_at)| {
                    let mappings: Vec<(String, String)> =
                        serde_json::from_str(&mappings_json).unwrap_or_default();
                    Ok(SessionImport {
                        id,
                        file_id,
                        source_type,
                        source_name,
                        row_count: row_count as usize,
                        mappings,
                        imported_at,
                    })
                },
            )
            .collect()
    }

    pub fn get_rows(&self, import_id: &str) -> Result<Vec<HashMap<String, String>>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM session_rows \
                 WHERE import_id = ?1 ORDER BY row_index ASC",
            )
            .map_err(|e| e.to_string())?;

        let jsons = stmt
            .query_map(rusqlite::params![import_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        jsons
            .into_iter()
            .map(|json| {
                serde_json::from_str::<HashMap<String, String>>(&json)
                    .map_err(|e| format!("Corrupt session row: {e}"))
            })
            .collect()
    }

    pub fn delete_import(&self, import_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM session_rows WHERE import_id = ?1",
                rusqlite::params![import_id],
            )
            .map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "DELETE FROM session_imports WHERE id = ?1",
                rusqlite::params![import_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
