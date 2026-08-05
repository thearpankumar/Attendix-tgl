mod admin;
mod admin_security;
mod client_log;
mod config;
mod device;
mod honeypot;
mod short_link;
mod student;

use axum::{extract::State, routing::get, Json, Router};
use axum_prometheus::{metrics_exporter_prometheus::PrometheusHandle, PrometheusMetricLayer};
use chrono::Utc;
use serde::Serialize;
use std::sync::{Arc, OnceLock};

use crate::constants::MAX_REQUEST_BODY_BYTES;
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

/// Public storage capability advertisement.
///
/// The frontends gate their direct-S3-upload path on `supportsDirectUpload`,
/// so that field must stay. The bucket name and region used to be returned
/// alongside it to any anonymous caller — infrastructure disclosure with no
/// client use — and have been dropped.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    provider: String,
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

    // Prometheus scrapes this; browsers must not. It exposes per-route request
    // volumes and timing for the whole instance and previously had no auth at
    // all. Scoped to trusted proxies (the monitoring network) plus an optional
    // bearer token.
    let metrics_routes = Router::new()
        .route(
            "/metrics",
            get(move || async move { metric_handle.render() }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::metrics_auth_middleware,
        ));

    let router = Router::new()
        .route("/health", get(health_check))
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .merge(metrics_routes)
        .nest("/api", api_routes)
        .merge(honeypot::create_routes())
        .layer(metric_layer)
        // Attendance bodies are small JSON documents; photos go to S3
        // out-of-band. The Excel roster upload raises this locally. Without a
        // limit anywhere, an unauthenticated caller could stream arbitrarily
        // large bodies into the multipart and JSON parsers.
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        // Outermost: any IP already on the deny-list (touched a honeypot path
        // or the canary admin account) is tarpitted then blocked before any
        // other middleware or handler runs.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::deny_list_middleware,
        ))
        .with_state(state);

    apply_security_headers(router)
}

/// Response headers applied to every route.
///
/// These live here rather than in `main.rs` so the test suite exercises the
/// real values. They were previously added to the binary's router only, and
/// the security tests asserted against a hardcoded copy of the CSP string —
/// so those tests passed no matter what the service actually sent.
pub fn apply_security_headers(router: Router) -> Router {
    use axum::http::{header::HeaderName, HeaderValue};
    use tower_http::set_header::SetResponseHeaderLayer;

    const HEADERS: &[(&str, &str)] = &[
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("x-xss-protection", "1; mode=block"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
        (
            "permissions-policy",
            "geolocation=(self), camera=(self), microphone=()",
        ),
        ("content-security-policy", API_CONTENT_SECURITY_POLICY),
    ];

    HEADERS.iter().fold(router, |router, (name, value)| {
        router.layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        ))
    })
}

/// CSP for API responses.
///
/// These are JSON documents; nothing here needs to execute script, load styles
/// or embed images, so the policy is maximally restrictive. It previously
/// allowed `script-src 'self' 'unsafe-inline'`, which is meaningless for a JSON
/// endpoint and weakened the policy for no benefit. The frontends get their own
/// (necessarily looser) policy from Caddy.
pub const API_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; \
     frame-ancestors 'none'; \
     base-uri 'none'; \
     form-action 'none'; \
     object-src 'none'";

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "OK".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

async fn health_ready(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let db_status = if sqlx::query("SELECT 1").execute(&state.db).await.is_ok() {
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
            supports_direct_upload: true,
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["supportsDirectUpload"], true);
        assert_eq!(json["provider"], "s3");
        // Infrastructure details must not be advertised publicly.
        assert!(json.get("bucket").is_none());
        assert!(json.get("region").is_none());
        // The old snake_case key must not leak onto the wire.
        assert!(json.get("supports_direct_upload").is_none());
    }

    #[test]
    fn non_s3_provider_does_not_support_direct_upload() {
        let info = StorageInfo {
            provider: "cloudinary".to_string(),
            supports_direct_upload: "cloudinary" == "s3",
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["supportsDirectUpload"], false);
    }
}
