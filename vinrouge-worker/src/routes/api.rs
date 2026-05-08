use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::{executor, state::AppState};

pub async fn project_detail(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let projects = state.projects.read().await;
    let loaded = match projects.get(&project_id) {
        Some(p) => p.clone(),
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "project not found"}))).into_response(),
    };
    drop(projects);

    let scripts = state.scripts.read().await;
    let project_scripts = scripts.get(&project_id).cloned().unwrap_or_default();
    drop(scripts);

    let scripts_json: Vec<serde_json::Value> = project_scripts.iter().map(|s| json!({
        "id": s.id,
        "label": s.label,
        "control_ref": s.control_ref,
        "created_at": s.created_at,
    })).collect();

    Json(json!({
        "id": loaded.project.id,
        "name": loaded.project.name,
        "created_at": loaded.project.created_at,
        "scripts": scripts_json,
    })).into_response()
}

pub async fn run_script(
    State(state): State<Arc<AppState>>,
    Path((project_id, script_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let projects = state.projects.read().await;
    if projects.get(&project_id).is_none() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "project not found"}))).into_response();
    }
    drop(projects);

    let script = {
        let scripts = state.scripts.read().await;
        scripts.get(&project_id)
            .and_then(|list| list.iter().find(|s| s.id == script_id).cloned())
    };
    let script = match script {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "script not found"}))).into_response(),
    };

    let datasource = state.datasources.read().await.get(&project_id).cloned();
    let datasource = match datasource {
        Some(ds) => ds,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "datasource not ready"}))).into_response(),
    };

    let script_text = script.script_text.clone();
    let results = match tokio::task::spawn_blocking(move || executor::run_script(&script_text, datasource))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    let json_results: Vec<serde_json::Value> = results.iter()
        .enumerate()
        .map(|(i, r)| result_to_json(i, r))
        .collect();

    // Refresh the cached results so the dashboard reflects the latest run.
    let cache_key = format!("{}/{}", project_id, script_id);
    state.results.write().await.insert(cache_key, json_results.clone());

    Json(json!({
        "script_id": script.id,
        "label": script.label,
        "results": json_results,
    })).into_response()
}

pub fn result_to_json(index: usize, r: &vinrouge::dsl::StatementResult) -> serde_json::Value {
    use vinrouge::dsl::StatementResult;
    match r {
        StatementResult::Assert(a) => json!({
            "kind": "assert", "index": index,
            "label": a.label, "passed": a.passed,
            "lhs_value": a.lhs_value, "rhs_value": a.rhs_value, "op": a.op,
        }),
        StatementResult::Sample(s) => json!({
            "kind": "sample", "index": index,
            "method": format!("{:?}", s.method),
            "population_size": s.population_size,
            "selected_count": s.selected.len(),
            "selected_indices": s.selected,
        }),
        StatementResult::Relation { from, to } => json!({
            "kind": "relation", "index": index, "from": from, "to": to,
        }),
        StatementResult::Value(v) => json!({
            "kind": "value", "index": index, "value": v,
        }),
        StatementResult::Error(e) => json!({
            "kind": "error", "index": index, "error": e,
        }),
        StatementResult::Chart(c) => json!({
            "kind": "chart", "index": index,
            "label": c.label, "chart_type": c.chart_type,
            "labels": c.labels, "values": c.values,
        }),
        StatementResult::Section(s) => json!({
            "kind": "section", "index": index,
            "title": s.title, "passed": s.passed, "failed": s.failed, "errors": s.errors,
            "results": s.results.iter().enumerate().map(|(j, inner)| result_to_json(j, inner)).collect::<Vec<_>>(),
        }),
        StatementResult::Schema(tables) => json!({
            "kind": "schema", "index": index,
            "tables": tables.iter().map(|t| json!({
                "name": t.name, "row_count": t.row_count,
                "columns": t.columns.iter().map(|c| json!({"name": c.name, "type": c.col_type})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
    }
}

pub use result_to_json as to_json;
