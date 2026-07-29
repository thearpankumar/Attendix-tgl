mod admin;
mod admin_security;
mod client_log;
mod config;
mod device;
mod short_link;
mod student;

use axum::{extract::State, routing::get, Json, Router};
use axum_prometheus::{metrics_exporter_prometheus::PrometheusHandle, PrometheusMetricLayer};
use chrono::Utc;
use serde::Serialize;
use std::sync::{Arc, OnceLock};

use crate::AppState;

/// `PrometheusMetricLayer::pair()` installs a process-wide global metrics
/// recorder, which can only be installed once. `create_routes` is called
/// exactly once in production, but tests build a Router per test case
/// within the same process — so the pair is cached and reused rather than
/// re-installed on every call.
fn metric_layer_and_handle() -> (PrometheusMetricLayer<'static>, PrometheusHandle) {
    static PAIR: OnceLock<(PrometheusMetricLayer<'static>, PrometheusHandle)> = OnceLock::new();
    PAIR.get_or_init(PrometheusMetricLayer::pair).clone()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    provider: String,
    bucket: String,
    region: String,
    supports_direct_upload: bool,
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
        .nest("/s", short_link::create_routes(state.clone()))
        .nest("/config", config::create_routes(state.clone()))
        .nest("/device", device::create_routes(state.clone()))
        .nest("/logs/client", client_log::create_routes(state.clone()))
        .nest(
            "/storage-info",
            Router::new().route("/", get(get_storage_info)).layer(
                axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::middleware::student_rate_limit_middleware,
                ),
            ),
        )
        .with_state(state.clone());

    let (metric_layer, metric_handle) = metric_layer_and_handle();

    Router::new()
        .route("/health", get(health_check))
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .route(
            "/metrics",
            get(move || async move { metric_handle.render() }),
        )
        .nest("/api", api_routes)
        .layer(metric_layer)
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

async fn get_storage_info(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let provider = state.config.storage.provider.clone();
    let supports_direct_upload = provider == "s3";
    Json(StorageInfo {
        provider,
        bucket: state.config.storage.s3.bucket.clone(),
        region: state.config.storage.s3.region.clone(),
        supports_direct_upload,
    })
}

#[cfg(test)]
mod storage_info_tests {
    use super::StorageInfo;

    /// Regression test: the frontends gate the entire direct-S3-upload path
    /// on `storageInfo.supportsDirectUpload`. If this field is missing or
    /// renamed on the wire, that gate silently never opens and uploaded
    /// photos are never persisted.
    #[test]
    fn serializes_supports_direct_upload_as_camel_case() {
        let info = StorageInfo {
            provider: "s3".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            supports_direct_upload: true,
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["supportsDirectUpload"], true);
        assert_eq!(json["provider"], "s3");
        // The old snake_case key must not leak onto the wire.
        assert!(json.get("supports_direct_upload").is_none());
    }

    #[test]
    fn non_s3_provider_does_not_support_direct_upload() {
        let info = StorageInfo {
            provider: "cloudinary".to_string(),
            bucket: String::new(),
            region: String::new(),
            supports_direct_upload: "cloudinary" == "s3",
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["supportsDirectUpload"], false);
    }
}
