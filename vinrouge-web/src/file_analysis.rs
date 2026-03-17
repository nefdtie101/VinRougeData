use js_sys::Uint8Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use vinrouge::analysis::{RelationshipDetector, Workflow, WorkflowDetector};
use vinrouge::schema::{Relationship, Table};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource};
use crate::types::AnalysisResult;

// ── Browser file analysis (WASM) ──────────────────────────────────────────────

pub async fn analyze_bytes(bytes: Vec<u8>, name: &str) -> Result<AnalysisResult, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();

    let tables: Vec<Table> = if ext == "csv" {
        CsvSource::from_bytes(bytes, name.to_string())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else if ext == "xlsx" || ext == "xls" {
        ExcelSource::from_bytes(bytes, name.to_string())
            .extract_schema()
            .await
            .map_err(|e| e.to_string())?
    } else {
        return Err(format!("Unsupported file type: .{ext}"));
    };

    let relationships = RelationshipDetector::new(tables.clone()).detect_relationships();
    let workflows = WorkflowDetector::new(tables.clone(), relationships.clone()).detect_workflows();

    Ok(AnalysisResult {
        tables,
        relationships,
        workflows,
    })
}

pub async fn read_file_bytes(file: &web_sys::File) -> Result<Vec<u8>, JsValue> {
    let buf = JsFuture::from(file.array_buffer()).await?;
    Ok(Uint8Array::new(&buf).to_vec())
}
