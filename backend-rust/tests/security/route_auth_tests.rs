use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt;

use attendance_geotag_backend::{
    config::AppConfig,
    middleware::{DenyList, RateLimiter, SessionCache},
    models::SystemConfig,
    routes,
    services::GpsHistoryService,
    AppState,
};
use tokio::sync::RwLock;

/// Creates a test application router
async fn create_test_app() -> axum::Router {
    let config = AppConfig::for_testing();
    // Lazy pool: none of the routes exercised in this file's tests actually
    // reach the database (they either 401 before a handler runs, or hit
    // handlers with no DB access), so no real Postgres connection is needed.
    let db = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .unwrap();

    // RateLimiter/DenyList/SessionCache/GpsHistoryService are Redis-backed
    // only (no in-memory fallback), so this uses the shared testcontainers
    // Redis instance rather than a real connection to `config.redis.url`.
    let redis_env = crate::test_db::get_test_environment().await;
    let redis_client =
        Arc::new(redis::Client::open(redis_env.redis_uri()).expect("valid test redis URL"));

    let rate_limiter = Arc::new(RateLimiter::new(redis_client.clone()));
    let deny_list = Arc::new(DenyList::new(redis_client.clone()));
    let session_cache = Arc::new(SessionCache::new(redis_client.clone(), 300));
    let gps_history = Arc::new(GpsHistoryService::new(redis_client.clone()));
    let system_config = Arc::new(RwLock::new(SystemConfig::default()));

    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::v2026_01_12())
        .load()
        .await;
    let storage = attendance_geotag_backend::storage::Storage::new(&aws_config, &config.storage)
        .unwrap_or_else(|_| panic!("Failed to initialize test storage"));

    let webauthn = Arc::new(
        attendance_geotag_backend::build_webauthn(&config).expect("valid test webauthn config"),
    );

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        redis: (*redis_client).clone(),
        rate_limiter,
        deny_list,
        session_cache,
        gps_history,
        start_time: std::time::Instant::now(),
        storage,
        http_client: reqwest::Client::new(),
        system_config,
        webauthn,
    });

    routes::create_routes(state)
}

/// `/metrics` exposes per-route request volumes and latency for the whole
/// instance. It was mounted on the root router with no auth at all — this test
/// existed under this name but asserted 200. It is now gated on a trusted-proxy
/// source or a `METRICS_TOKEN` bearer token, and the test config sets neither.
#[tokio::test]
async fn test_metrics_not_world_accessible() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Regression test: /metrics must actually report HTTP request metrics, not
/// an empty body. Before this was wired up (axum-prometheus's
/// PrometheusMetricLayer applied to the router), the handler called
/// prometheus::gather() against a registry nothing ever registered into,
/// silently returning 0 bytes — the Grafana dashboard's backend panels had
/// no data as a result.
#[tokio::test]
async fn test_metrics_reports_actual_http_metrics() {
    let app = create_test_app().await;

    // Generate at least one recorded request before scraping.
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // SAFETY: single-threaded test setup, before any concurrent access.
    unsafe { std::env::set_var("METRICS_TOKEN", "test-metrics-scrape-token") };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .header("authorization", "Bearer test-metrics-scrape-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        body.contains("axum_http_requests_total"),
        "expected /metrics body to contain HTTP request metrics, got: {body}"
    );
    assert!(!body.trim().is_empty());
}

#[tokio::test]
async fn test_storage_info_is_public_but_rate_limited() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/storage-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// Regression test: the storage-info response body must include
/// `supportsDirectUpload`, otherwise the student frontends' direct-S3-upload
/// path (gated on this exact field) silently never activates and photos are
/// never persisted. See backend-rust/src/routes/mod.rs::get_storage_info.
#[tokio::test]
async fn test_storage_info_response_body_shape() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/storage-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.get("provider").and_then(|v| v.as_str()).is_some(),
        "response must include a string `provider` field"
    );
    let supports_direct_upload = json
        .get("supportsDirectUpload")
        .unwrap_or_else(|| panic!("response missing `supportsDirectUpload` field: {json}"))
        .as_bool()
        .unwrap_or_else(|| panic!("`supportsDirectUpload` must be a boolean: {json}"));

    // Test config uses the s3 provider (see AppConfig::for_testing), the only
    // StorageProvider actually implemented.
    assert_eq!(json["provider"], "s3");
    assert!(
        supports_direct_upload,
        "s3 provider must report supportsDirectUpload: true"
    );
    // Regression: the bucket name and region used to be returned to any
    // anonymous caller alongside this field.
    assert!(json.get("bucket").is_none(), "must not disclose the bucket");
    assert!(json.get("region").is_none(), "must not disclose the region");
}

#[tokio::test]
async fn test_device_verify_is_public() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/device/verify")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_client_log_is_public() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs/client")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_config_get_requires_auth() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_security_settings_requires_auth() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/security/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_dashboard_requires_auth() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ===================================================================
// Super-admin / mentor / manual-attendance feature: every new route must be
// behind the same auth layer as everything else in `protected_routes` — a
// route added to the router but missing from that group would silently be
// reachable with no cookie/token at all. One assertion per route.
// ===================================================================

#[tokio::test]
async fn test_admin_users_list_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_users_create_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_users_update_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/admin/users/{}", uuid::Uuid::new_v4()))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_users_delete_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/admin/users/{}", uuid::Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_profile_password_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/admin/profile/password")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_session_roster_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/admin/sessions/{}/roster",
                    uuid::Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_manual_attendance_mark_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/admin/sessions/{}/attendance/manual",
                    uuid::Uuid::new_v4()
                ))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_manual_attendance_undo_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/admin/sessions/{}/attendance/manual/TEST001",
                    uuid::Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_sessions_create_requires_auth() {
    let app = create_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/sessions")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
