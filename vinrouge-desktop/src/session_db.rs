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
use vinrouge::analysis::{DataProfiler, PatternType};
use vinrouge::schema::{Column as SchemaColumn, DataType as SchemaDataType, RelationshipType, Table as SchemaTable};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};
use vinrouge::RelationshipDetector;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelCandidate {
    pub left_import_id: String,
    pub left_table: String,
    pub left_col: String,
    pub right_import_id: String,
    pub right_table: String,
    pub right_col: String,
    pub confidence: u8,      // 0–100
    pub overlap_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSpec {
    pub left_import_id: String,
    pub left_col: String,
    pub right_import_id: String,
    pub right_col: String,
}

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
    /// Columns with a mapping are stored under the PBC field name; columns with
    /// no mapping or an empty target are kept under their original header name.
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
                let target = lookup.get(hdr.as_str()).copied().unwrap_or("");
                let key = if target.is_empty() { hdr.as_str() } else { target };
                out.insert(key.to_string(), val.clone());
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

        // Store rows with ORIGINAL column names. PBC renaming is applied only
        // when building the master record so non-master tabs show source schema.
        let rows: Vec<HashMap<String, String>> = data
            .iter()
            .map(|row| {
                headers.iter().zip(row.iter())
                    .filter(|(h, _)| !h.trim().is_empty())
                    .map(|(h, v)| (h.clone(), v.clone()))
                    .collect()
            })
            .filter(|m: &HashMap<String, String>| !m.is_empty())
            .collect();

        self.write_import(file_id, "csv", name, mappings, rows)
    }

    // ── Excel import ──────────────────────────────────────────────────────────

    /// Import an Excel file. Each sheet becomes its own import/table.
    /// Returns the list of import IDs created (one per sheet).
    pub fn import_excel(
        &self,
        file_id: Option<&str>,
        name: &str,
        bytes: Vec<u8>,
        mappings: &[(String, String)],
        sheet: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;

        // Collect (sheet_name, rows) pairs — each sheet gets its own import.
        let sheets: Vec<(String, Vec<HashMap<String, String>>)> = rt.block_on(async {
            let src = ExcelSource::from_bytes(bytes.clone(), name.to_string());

            let sheet_names: Vec<String> = match sheet {
                Some(sh) => vec![sh.to_string()],
                None => src.sheet_names().map_err(|e| e.to_string())?,
            };

            let mut result = Vec::new();

            for sh in &sheet_names {
                let mut src = ExcelSource::from_bytes(bytes.clone(), name.to_string())
                    .with_sheet(sh.clone());

                let tables = src.extract_schema().await.map_err(|e| e.to_string())?;
                let headers: Vec<String> = tables
                    .into_iter()
                    .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                    .collect();

                if headers.is_empty() {
                    continue; // skip empty/hidden sheets
                }

                let data = src.read_data().await.map_err(|e| e.to_string())?;
                // Store rows with ORIGINAL column names. PBC renaming is applied only
                // when building the master record so non-master tabs show source schema.
                let rows: Vec<HashMap<String, String>> = data
                    .iter()
                    .map(|row| {
                        headers.iter().zip(row.iter())
                            .filter(|(h, _)| !h.trim().is_empty())
                            .map(|(h, v)| (h.clone(), v.clone()))
                            .collect()
                    })
                    .filter(|m: &HashMap<String, String>| !m.is_empty())
                    .collect();

                if !rows.is_empty() {
                    result.push((sh.clone(), rows));
                }
            }

            Ok::<_, String>(result)
        })?;

        // Build a source name per sheet: "filename.xlsx [SheetName]"
        // so each sheet appears as a distinct table in the UI.
        let stem = std::path::Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name);

        let mut import_ids = Vec::new();
        for (sheet_name, rows) in sheets {
            let source_name = if sheet_name == stem || sheets_count_is_one(sheet, name) {
                name.to_string()
            } else {
                format!("{} [{}]", name, sheet_name)
            };
            let id = self.write_import(file_id, "excel", &source_name, mappings, rows)?;
            import_ids.push(id);
        }

        if import_ids.is_empty() {
            return Err("No data found in any sheet".to_string());
        }

        Ok(import_ids)
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

    pub fn get_rows_paged(
        &self,
        import_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<HashMap<String, String>>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM session_rows \
                 WHERE import_id = ?1 ORDER BY row_index ASC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| e.to_string())?;
        let jsons = stmt
            .query_map(rusqlite::params![import_id, limit as i64, offset as i64], |row| {
                row.get::<_, String>(0)
            })
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

    // ── Column keys from first row ─────────────────────────────────────────────

    pub fn get_import_columns(&self, import_id: &str) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM session_rows WHERE import_id = ?1 LIMIT 1")
            .map_err(|e| e.to_string())?;
        match stmt.query_row(rusqlite::params![import_id], |r| r.get::<_, String>(0)) {
            Ok(json) => {
                let map: HashMap<String, String> =
                    serde_json::from_str(&json).map_err(|e| format!("Corrupt row: {e}"))?;
                let mut cols: Vec<String> = map.into_keys().collect();
                cols.sort();
                Ok(cols)
            }
            Err(_) => Ok(vec![]),
        }
    }

    fn get_rows_sample(
        &self,
        import_id: &str,
        limit: usize,
    ) -> Result<Vec<HashMap<String, String>>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM session_rows \
                 WHERE import_id = ?1 ORDER BY row_index ASC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let jsons: Vec<String> = stmt
            .query_map(rusqlite::params![import_id, limit as i64], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        jsons
            .into_iter()
            .map(|json| {
                serde_json::from_str::<HashMap<String, String>>(&json)
                    .map_err(|e| format!("Corrupt row: {e}"))
            })
            .collect()
    }

    fn sample_column(
        &self,
        import_id: &str,
        col: &str,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let rows = self.get_rows_sample(import_id, limit)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.get(col).cloned())
            .filter(|v| !v.trim().is_empty())
            .collect())
    }

    // ── Relationship detection ─────────────────────────────────────────────────

    /// Build a `SchemaTable` from a session import by profiling its stored rows.
    fn build_schema_table(&self, imp: &SessionImport) -> Result<SchemaTable, String> {
        // Column names in a stable order: use mappings (tgt if mapped, src if not)
        let col_names: Vec<String> = imp
            .mappings
            .iter()
            .map(|(src, tgt)| if tgt.is_empty() { src.clone() } else { tgt.clone() })
            .filter(|n| !n.trim().is_empty())
            .collect();
        if col_names.is_empty() {
            return Err(format!("Import '{}' has no columns", imp.source_name));
        }

        // Load up to 1 000 rows and convert HashMap → ordered Vec<Vec<String>>
        let raw = self.get_rows_sample(&imp.id, 1000)?;
        let ordered: Vec<Vec<String>> = raw
            .iter()
            .map(|r| {
                col_names
                    .iter()
                    .map(|k| r.get(k).cloned().unwrap_or_default())
                    .collect()
            })
            .collect();

        // Build placeholder Column objects (DataProfiler only needs the name)
        let schema_cols: Vec<SchemaColumn> = col_names
            .iter()
            .map(|n| SchemaColumn::new(n.clone(), SchemaDataType::VarChar { max_length: None }))
            .collect();

        let profiler = DataProfiler::new(1000);
        let profile = profiler.profile_data(&ordered, &schema_cols);

        let table_name = stem_from_source(&imp.source_name);
        let mut table =
            SchemaTable::new(table_name, imp.source_type.clone(), imp.source_name.clone());
        table.row_count = Some(imp.row_count);

        for cp in &profile.column_profiles {
            let mut col =
                SchemaColumn::new(cp.column_name.clone(), SchemaDataType::VarChar { max_length: None });
            col.unique_count = Some(cp.unique_values);
            col.null_count = Some(cp.null_count);
            // Mark as PK heuristic: high uniqueness AND looks like an identifier
            col.is_primary_key = cp.data_patterns.contains(&PatternType::UniqueIdentifier)
                && cp.distinct_ratio > 0.9;
            table.add_column(col);
        }

        Ok(table)
    }

    /// Detect join relationships across all non-master imports using:
    ///  - `RelationshipDetector` (name + FK pattern matching)
    ///  - Value overlap sampling for scoring
    /// Only returns relationships where one side is the primary table
    /// (the import with the most rows), so secondary-to-secondary joins
    /// are never surfaced.
    pub fn detect_data_relationships(&self) -> Result<Vec<RelCandidate>, String> {
        let imports = self.list_imports()?;
        let imports: Vec<&SessionImport> = imports
            .iter()
            .filter(|i| i.source_type != "master")
            .collect();

        if imports.len() < 2 {
            return Ok(vec![]);
        }

        // The primary table is the one with the most rows.
        let primary_id = imports
            .iter()
            .max_by_key(|i| i.row_count)
            .map(|i| i.id.clone())
            .unwrap_or_default();

        let mut tables: Vec<SchemaTable> = Vec::new();
        let mut table_to_import: HashMap<String, String> = HashMap::new();

        for imp in &imports {
            match self.build_schema_table(imp) {
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

        let mut detector = RelationshipDetector::new(tables);
        let relationships = detector.detect_relationships();

        let mut seen: std::collections::HashSet<(String, String, String, String)> =
            std::collections::HashSet::new();
        let mut candidates: Vec<RelCandidate> = Vec::new();

        for rel in &relationships {
            let Some(lid) = table_to_import.get(&rel.from_table) else { continue };
            let Some(rid) = table_to_import.get(&rel.to_table) else { continue };

            // Only keep relationships that involve the primary table on one side.
            if lid != &primary_id && rid != &primary_id {
                continue;
            }

            let key = (lid.clone(), rel.from_column.clone(), rid.clone(), rel.to_column.clone());
            if !seen.insert(key) {
                continue;
            }

            let name_conf: u8 = match &rel.relationship_type {
                RelationshipType::ForeignKey => 95,
                RelationshipType::NameMatch { confidence } => *confidence,
                RelationshipType::UniquePattern => 70,
                RelationshipType::Composite => 85,
                RelationshipType::ValueOverlap { overlap_percent } => *overlap_percent,
            };

            // Value overlap on up to 200 sample rows
            let lv = self.sample_column(lid, &rel.from_column, 200)?;
            let rv = self.sample_column(rid, &rel.to_column, 200)?;
            let rv_set: std::collections::HashSet<&str> =
                rv.iter().map(|s| s.as_str()).collect();
            let overlap = lv.iter().filter(|v| rv_set.contains(v.as_str())).count();
            let overlap_ratio = if lv.is_empty() {
                0.0_f64
            } else {
                overlap as f64 / lv.len().min(rv.len()).max(1) as f64
            };

            // 60 % name / pattern confidence + 40 % value overlap
            let confidence = ((name_conf as f64 * 0.6) + (overlap_ratio * 100.0 * 0.4)) as u8;

            candidates.push(RelCandidate {
                left_import_id: lid.clone(),
                left_table: rel.from_table.clone(),
                left_col: rel.from_column.clone(),
                right_import_id: rid.clone(),
                right_table: rel.to_table.clone(),
                right_col: rel.to_column.clone(),
                confidence: confidence.min(100),
                overlap_count: overlap,
            });
        }

        candidates.sort_by(|a, b| b.confidence.cmp(&a.confidence));
        Ok(candidates)
    }

    // ── Master record builder ──────────────────────────────────────────────────

    /// Hash-join the imports described by `joins` into a single "master" import.
    /// The import with the most rows is used as the primary (left) table.
    /// Returns the import_id of the new master record.
    pub fn build_master_record(&self, joins: Vec<JoinSpec>) -> Result<String, String> {
        if joins.is_empty() {
            return Err("No joins specified".to_string());
        }

        let imports = self.list_imports()?;
        let all_ids: std::collections::HashSet<String> = joins
            .iter()
            .flat_map(|j| [j.left_import_id.clone(), j.right_import_id.clone()])
            .collect();
        let relevant: Vec<&SessionImport> = imports
            .iter()
            .filter(|i| all_ids.contains(&i.id) && i.source_type != "master")
            .collect();

        if relevant.is_empty() {
            return Err("No valid imports to join".to_string());
        }

        let primary = relevant
            .iter()
            .max_by_key(|i| i.row_count)
            .copied()
            .unwrap();

        let tname: HashMap<String, String> = imports
            .iter()
            .map(|i| (i.id.clone(), stem_from_source(&i.source_name)))
            .collect();

        let mut all_rows: HashMap<String, Vec<HashMap<String, String>>> = HashMap::new();
        for imp in &relevant {
            all_rows.insert(imp.id.clone(), self.get_rows(&imp.id)?);
        }

        // Build lookup maps: secondary join-key value → row
        struct Lookup {
            primary_col: String,
            secondary_col: String,
            map: HashMap<String, HashMap<String, String>>,
            prefix: String,
        }

        let mut lookups: Vec<Lookup> = Vec::new();
        for join in &joins {
            let (sec_id, sec_col, pri_col) = if join.left_import_id == primary.id {
                (&join.right_import_id, &join.right_col, &join.left_col)
            } else if join.right_import_id == primary.id {
                (&join.left_import_id, &join.left_col, &join.right_col)
            } else {
                continue;
            };

            let sec_rows = all_rows.get(sec_id).cloned().unwrap_or_default();
            let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
            for row in sec_rows {
                if let Some(k) = row.get(sec_col).cloned() {
                    let k = k.trim().to_lowercase();
                    if !k.is_empty() {
                        map.entry(k).or_insert(row);
                    }
                }
            }
            let prefix = tname
                .get(sec_id)
                .cloned()
                .unwrap_or_else(|| sec_id.chars().take(8).collect());
            lookups.push(Lookup {
                primary_col: pri_col.clone(),
                secondary_col: sec_col.clone(),
                map,
                prefix,
            });
        }

        let primary_rows = all_rows.remove(&primary.id).unwrap_or_default();
        let mut merged: Vec<HashMap<String, String>> =
            Vec::with_capacity(primary_rows.len());

        for mut row in primary_rows {
            for lookup in &lookups {
                let key = row
                    .get(&lookup.primary_col)
                    .cloned()
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                if let Some(sec_row) = lookup.map.get(&key) {
                    for (k, v) in sec_row {
                        if k == &lookup.secondary_col {
                            continue;
                        }
                        let dest = if row.contains_key(k.as_str()) {
                            format!("{}_{}", lookup.prefix, k)
                        } else {
                            k.clone()
                        };
                        row.insert(dest, v.clone());
                    }
                }
            }
            merged.push(row);
        }

        // Apply primary table's PBC column mapping so master has standardised field names.
        let pbc_map: HashMap<&str, &str> = primary.mappings
            .iter()
            .filter(|(src, tgt)| !src.is_empty() && !tgt.is_empty())
            .map(|(src, tgt)| (src.as_str(), tgt.as_str()))
            .collect();

        let final_rows: Vec<HashMap<String, String>> = if pbc_map.is_empty() {
            merged
        } else {
            merged.into_iter().map(|row| {
                row.into_iter().map(|(k, v)| {
                    let new_k = pbc_map.get(k.as_str()).copied().unwrap_or(k.as_str());
                    (new_k.to_string(), v)
                }).collect()
            }).collect()
        };

        // Remove any previous master record
        for imp in imports.iter().filter(|i| i.source_type == "master") {
            self.delete_import(&imp.id)?;
        }

        self.write_import(None, "master", "Master Record", &[], final_rows)
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sheets_count_is_one(sheet: Option<&str>, _name: &str) -> bool {
    sheet.is_some() // if a specific sheet was requested, it's a single-sheet import
}

fn stem_from_source(source_name: &str) -> String {
    std::path::Path::new(source_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(source_name)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}
