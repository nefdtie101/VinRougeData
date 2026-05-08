use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use minijinja::{Environment, Value as JinjaValue};
use serde_json::json;

use crate::{renderer, state::AppState};
use super::api::to_json;

fn make_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    env.add_template_owned("base.html",      include_str!("../../templates/base.html").to_string()).unwrap();
    env.add_template_owned("dashboard.html", include_str!("../../templates/dashboard.html").to_string()).unwrap();
    env.add_template_owned("home.html",      include_str!("../../templates/home.html").to_string()).unwrap();
    env
}

/// Home — renders every project and every cached script result as one page.
/// Scripts and results were fixed at startup from the .vrd files.
pub async fn home(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let projects_snap: Vec<_> = {
        let p = state.projects.read().await;
        p.values().cloned().collect()
    };
    let scripts_snap = state.scripts.read().await;
    let results_snap = state.results.read().await;

    let mut sections: Vec<serde_json::Value> = Vec::new();

    for lp in &projects_snap {
        let scripts = scripts_snap.get(&lp.project.id).cloned().unwrap_or_default();

        let script_panels: Vec<serde_json::Value> = scripts.iter().map(|s| {
            let key = format!("{}/{}", lp.project.id, s.id);
            let content_html = results_snap
                .get(&key)
                .map(|r| renderer::render_results(r))
                .unwrap_or_else(|| "<p class=\"vr-empty\">No results.</p>".into());

            json!({
                "script_id":    s.id,
                "label":        s.label,
                "content_html": content_html,
            })
        }).collect();

        sections.push(json!({
            "project_id":   lp.project.id,
            "project_name": lp.project.name,
            "scripts":      script_panels,
        }));
    }

    let env = make_env();
    let tmpl = env.get_template("home.html").unwrap();
    let ctx = minijinja::context! { sections => JinjaValue::from_serialize(&sections) };
    match tmpl.render(ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Per-script dashboard — reads from cache. Cache is populated at startup.
pub async fn serve_dashboard(
    State(state): State<Arc<AppState>>,
    Path((project_id, script_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let loaded = {
        let p = state.projects.read().await;
        match p.get(&project_id) {
            Some(lp) => lp.clone(),
            None => return (StatusCode::NOT_FOUND, Html("<h1>Project not found</h1>".to_string())).into_response(),
        }
    };

    let script = {
        let scripts = state.scripts.read().await;
        scripts.get(&project_id)
            .and_then(|list| list.iter().find(|s| s.id == script_id).cloned())
    };
    let script = match script {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, Html("<h1>Script not found</h1>".to_string())).into_response(),
    };

    let cache_key = format!("{project_id}/{script_id}");
    let json_results = match state.results.read().await.get(&cache_key).cloned() {
        Some(r) => r,
        None => {
            // Should not happen — results are populated at startup.
            // Fall back to running the script against the cached datasource.
            let datasource = match state.datasources.read().await.get(&project_id).cloned() {
                Some(ds) => ds,
                None => return (StatusCode::INTERNAL_SERVER_ERROR, Html("<p>Datasource not ready.</p>".to_string())).into_response(),
            };
            let script_text = script.script_text.clone();
            let raw = match tokio::task::spawn_blocking(move || crate::executor::run_script(&script_text, datasource))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            {
                Ok(r) => r,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("<p>DSL error: {e}</p>"))).into_response(),
            };
            raw.iter().enumerate().map(|(i, r)| to_json(i, r)).collect()
        }
    };

    let content_html = renderer::render_results(&json_results);
    let env = make_env();
    let tmpl = env.get_template("dashboard.html").unwrap();
    let ctx = minijinja::context! {
        project_name => loaded.project.name,
        script_label => script.label,
        content_html => content_html,
    };
    match tmpl.render(ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
