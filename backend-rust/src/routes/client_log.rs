use axum::{routing::post, Json, Router};
use std::sync::Arc;

use crate::AppState;

pub fn create_routes() -> Router<Arc<AppState>> {
    Router::new().route("/", post(log_client_error))
}

async fn log_client_error(
    Json(payload): Json<serde_json::Value>,
) -> impl axum::response::IntoResponse {
    tracing::warn!("Client error: {:?}", payload);
    Json(serde_json::json!({ "logged": true }))
}
