pub mod api;
mod dashboard;

use std::sync::Arc;
use axum::{routing::{get, post}, Router};

use crate::state::AppState;

pub fn make_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(dashboard::home))
        .route("/projects/:id", get(api::project_detail))
        .route("/projects/:id/scripts/:script_id/run", post(api::run_script))
        .route("/projects/:id/scripts/:script_id/dashboard", get(dashboard::serve_dashboard))
        .with_state(state)
}
