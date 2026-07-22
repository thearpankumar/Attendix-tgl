mod admin;
mod admin_security;
mod client_log;
mod config;
mod device;
mod short_link;
mod student;

use axum::{extract::State, routing::get, Json, Router};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Serialize)]
struct StorageInfo {
    provider: String,
    bucket: String,
    region: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct HealthReadyResponse {
    status: String,
    timestamp: String,
    database: String,
    redis: String,
}

pub fn create_routes(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        .nest("/admin", admin::create_routes(state.clone()))
        .nest(
            "/admin/security",
            admin_security::create_routes(state.clone()),
        )
        .nest("/attend", student::create_routes(state.clone()))
        .nest("/s", short_link::create_routes())
        .nest("/config", config::create_routes())
        .nest("/device", device::create_routes())
        .nest("/logs/client", client_log::create_routes())
        .route("/storage-info", get(get_storage_info))
        .with_state(state.clone());

    Router::new()
        .route("/health", get(health_check))
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .route("/metrics", get(metrics))
        .nest("/api", api_routes)
        .with_state(state)
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "OK".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

async fn health_ready(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let db_status = if state
        .db
        .database("admin")
        .run_command(mongodb::bson::doc! { "ping": 1 })
        .await
        .is_ok()
    {
        "connected"
    } else {
        "disconnected"
    };

    let redis_status = if let Some(ref redis_client) = state.redis {
        use redis::AsyncCommands;
        if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
            if conn.ping::<String>().await.is_ok() {
                "connected"
            } else {
                "disconnected"
            }
        } else {
            "disconnected"
        }
    } else {
        "not_configured"
    };

    let status = if db_status == "connected" {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(HealthReadyResponse {
            status: if db_status == "connected" {
                "OK".to_string()
            } else {
                "UNHEALTHY".to_string()
            },
            timestamp: Utc::now().to_rfc3339(),
            database: db_status.to_string(),
            redis: redis_status.to_string(),
        }),
    )
}

async fn health_live() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "alive".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

async fn metrics() -> impl axum::response::IntoResponse {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    (
        axum::http::StatusCode::OK,
        String::from_utf8(buffer).unwrap(),
    )
}

async fn get_storage_info(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    Json(StorageInfo {
        provider: state.config.storage.provider.clone(),
        bucket: state.config.storage.s3.bucket.clone(),
        region: state.config.storage.s3.region.clone(),
    })
}
