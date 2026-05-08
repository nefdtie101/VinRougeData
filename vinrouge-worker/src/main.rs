mod config;
mod executor;
mod renderer;
mod routes;
mod session_reader;
mod state;

use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vinrouge_worker=info,warn")),
        )
        .init();

    let cfg = config::WorkerConfig::load()?;
    let data_dir = std::path::PathBuf::from(&cfg.data_dir);
    std::fs::create_dir_all(&data_dir)?;

    let app_state = Arc::new(state::AppState::new());

    // ── 1. Scan .vrd files ────────────────────────────────────────────────────
    tracing::info!("Scanning {} for .vrd projects...", data_dir.display());
    match executor::scan_data_dir(&data_dir) {
        Ok(loaded) => {
            let mut projects = app_state.projects.write().await;
            for lp in loaded {
                projects.insert(lp.project.id.clone(), lp);
            }
            tracing::info!("Loaded {} project(s).", projects.len());
        }
        Err(e) => tracing::error!("Failed to scan data_dir: {e}"),
    }

    let project_ids: Vec<(String, state::LoadedProject)> = {
        let p = app_state.projects.read().await;
        p.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };

    for (project_id, lp) in project_ids {
        let pid = project_id.clone();

        // ── 2. Load scripts from the .vrd (one DB read, then cached) ─────────
        let project_dir = lp.project_dir.clone();
        let scripts = match tokio::task::spawn_blocking(move || {
            vinrouge::projects::list_dsl_scripts(&project_dir)
        }).await {
            Ok(Ok(s))  => s,
            Ok(Err(e)) => { tracing::error!("Script load error for {pid}: {e}"); continue; }
            Err(e)     => { tracing::error!("Script load panic for {pid}: {e}"); continue; }
        };
        tracing::info!("  {} script(s) loaded for project {pid}", scripts.len());
        app_state.scripts.write().await.insert(project_id.clone(), scripts.clone());

        // ── 3. Build datasource ───────────────────────────────────────────────
        let project_dir2 = lp.project_dir.clone();
        let pid2 = project_id.clone();
        let ds = match tokio::task::spawn_blocking(move || {
            executor::build_datasource(&project_dir2)
        }).await {
            Ok(Ok(ds)) => Arc::new(ds),
            Ok(Err(e)) => { tracing::error!("Datasource error for {pid2}: {e}"); continue; }
            Err(e)     => { tracing::error!("Datasource panic for {pid2}: {e}"); continue; }
        };
        app_state.datasources.write().await.insert(project_id.clone(), ds.clone());

        // ── 4. Run all scripts and cache results ──────────────────────────────
        let scripts2 = scripts.clone();
        let ds2 = ds.clone();
        let runs = tokio::task::spawn_blocking(move || executor::run_all_scripts(&scripts2, ds2))
            .await
            .unwrap_or_default();

        let mut cache = app_state.results.write().await;
        for run in runs {
            let key = format!("{}/{}", project_id, run.script_id);
            let json_results: Vec<serde_json::Value> = run.results.iter()
                .enumerate()
                .map(|(i, r)| routes::api::result_to_json(i, r))
                .collect();
            tracing::info!("  Cached '{}' ({} results)", run.label, json_results.len());
            cache.insert(key, json_results);
        }
    }

    let router = axum::Router::new()
        .merge(routes::make_router(app_state))
        .nest_service("/assets", ServeDir::new("assets"));

    let addr = format!("0.0.0.0:{}", cfg.port);
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
