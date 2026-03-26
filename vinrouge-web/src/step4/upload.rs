use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::file_analysis::{analyze_bytes, read_file_bytes};
use crate::ipc::tauri_invoke_args;
use crate::ollama::{ask_column_mapping, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{PbcGroup, ProjectFile};
use super::types::{DataFile, FileSource};

// ── Column mapping helpers ────────────────────────────────────────────────────

/// Fallback: normalised string-matching when AI is unavailable.
pub fn normalize_map(columns: &[String], all_fields: &[String]) -> Vec<(String, String)> {
    let norm = |s: &str| s.to_lowercase().replace(['_', ' ', '-'], "");
    columns
        .iter()
        .map(|col| {
            let target = all_fields
                .iter()
                .find(|f| norm(f) == norm(col))
                .cloned()
                .unwrap_or_default();
            (col.clone(), target)
        })
        .collect()
}

// ── Upload helper ─────────────────────────────────────────────────────────────
// Only headers are extracted at upload time.  The full file bytes are NOT sent
// to Tauri here — that happens later when "Proceed to 4a" is clicked so that
// the complete dataset is available for DSL math reconciliations in Step 4a/5.

pub fn start_file_upload(
    file: web_sys::File,
    uploading: RwSignal<bool>,
    status: RwSignal<String>,
    data_files: RwSignal<Vec<DataFile>>,
    pbc_groups: RwSignal<Vec<PbcGroup>>,
    selected_id: RwSignal<Option<String>>,
) {
    let name = file.name();
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    if !matches!(ext.as_str(), "csv" | "xlsx" | "xls") {
        status.set(format!(
            "Unsupported file type (.{ext}) — drop a CSV or Excel file"
        ));
        return;
    }
    uploading.set(true);

    // Keep a clone of the browser File so we can re-read bytes on "Proceed"
    let browser_file = file.clone();

    spawn_local(async move {
        // Read raw bytes from the browser File object — needed to extract headers
        let bytes = match read_file_bytes(&file).await {
            Ok(b) => b,
            Err(e) => {
                status.set(format!("Could not read file: {e:?}"));
                uploading.set(false);
                return;
            }
        };

        // Clone bytes before analyze_bytes consumes them; we need them to persist
        // the file immediately after mapping.
        let bytes_for_save = bytes.clone();

        // Parse column names in WASM.
        // For multi-sheet Excel files we flatten all sheets but deduplicate so the
        // same column name from different sheets is only mapped once.
        // Empty and whitespace-only headers (title rows, merged cells) are dropped.
        let columns = match analyze_bytes(bytes, &name).await {
            Ok(r) => {
                let mut seen = std::collections::HashSet::new();
                r.tables
                    .into_iter()
                    .flat_map(|t| t.columns.into_iter().map(|c| c.name))
                    .filter(|h| {
                        let trimmed = h.trim();
                        !trimmed.is_empty() && seen.insert(trimmed.to_string())
                    })
                    .collect::<Vec<_>>()
            }
            Err(e) => {
                status.set(format!("Could not parse headers: {e}"));
                uploading.set(false);
                return;
            }
        };

        // Map columns to PBC fields — try AI first, fall back to string matching.
        let groups_snap = pbc_groups.get_untracked();
        status.set("Mapping columns with AI…".to_string());
        let mappings = match ask_column_mapping(
            OLLAMA_DEFAULT_URL,
            OLLAMA_DEFAULT_MODEL,
            &columns,
            &groups_snap,
        )
        .await
        {
            Ok(m) if !m.is_empty() => m,
            _ => {
                let all_fields: Vec<String> = groups_snap
                    .iter()
                    .flat_map(|g| g.items.iter())
                    .flat_map(|i| i.fields.iter().cloned())
                    .collect();
                normalize_map(&columns, &all_fields)
            }
        };

        // Immediately save the file to the project and persist mappings so
        // they survive navigation away and back.
        let saved_source = match tauri_invoke_args::<ProjectFile>(
            "add_data_file",
            serde_json::json!({ "name": name, "bytes": bytes_for_save }),
        )
        .await
        {
            Ok(pf) => {
                let _ = tauri_invoke_args::<()>(
                    "save_column_mappings",
                    serde_json::json!({ "fileId": pf.id, "mappings": mappings }),
                )
                .await;
                FileSource::Saved(pf.id)
            }
            Err(_) => FileSource::Browser(browser_file.clone()),
        };

        let local_id = name.clone();
        data_files.update(|v| {
            let df = DataFile {
                local_id: local_id.clone(),
                name: name.clone(),
                columns,
                mappings,
                source: saved_source,
            };
            // Replace if same filename was already uploaded
            if let Some(existing) = v.iter_mut().find(|d| d.name == name) {
                *existing = df;
            } else {
                v.push(df);
            }
        });
        selected_id.set(Some(local_id));
        uploading.set(false);
    });
}

/// Upload all files from a FileList, one at a time.
pub fn upload_file_list(
    files: web_sys::FileList,
    uploading: RwSignal<bool>,
    status: RwSignal<String>,
    data_files: RwSignal<Vec<DataFile>>,
    pbc_groups: RwSignal<Vec<PbcGroup>>,
    selected_id: RwSignal<Option<String>>,
) {
    let n = files.length();
    for i in 0..n {
        if let Some(f) = files.get(i) {
            start_file_upload(f, uploading, status, data_files, pbc_groups, selected_id);
        }
    }
}
