//! End-to-end behavioural tests for the super-admin / mentor / manual-
//! attendance feature, exercised over real HTTP against a real Postgres +
//! Redis (testcontainers), the same way a browser would: login, read back
//! the CSRF cookie the safe-method GET issues, then send it as the
//! `x-csrf-token` header on every mutating request — exactly what
//! `routes/admin.rs`'s csrf -> auth -> rate-limit layer stack requires.
//!
//! This is the deepest tier: `route_auth_tests.rs` only checks "no token ->
//! 401" for these routes without touching the database; these tests instead
//! drive the actual business logic (bootstrap-then-lockdown, role gating,
//! self-lockout, manual-mark idempotency/conflict) end to end.

use axum::{
    body::Body,
    http::{header::SET_COOKIE, HeaderMap, Request, StatusCode},
};
// `file_serial`, not `serial`: `serial_test`'s plain `#[serial]` uses an
// in-process `Mutex`, which nextest's one-fresh-process-per-test model makes
// a no-op between these tests (each sees an uncontended fresh lock) — the
// `file_locks` feature just makes the OS-level-file-lock-backed
// `#[file_serial]` macro available, it does not change what `#[serial]`
// itself does. This was the actual cause of the `admin_bootstrap`-group
// tests interleaving and racing on the shared `admins`/`locations` tables on
// CI even after `file_locks` was added to Cargo.toml.
use serial_test::file_serial;
use std::sync::Arc;
use tower::ServiceExt;

use attendance_geotag_backend::{
    config::AppConfig,
    middleware::{DenyList, RateLimiter, SessionCache},
    models::{Admin, SystemConfig},
    routes,
    services::GpsHistoryService,
    AppState,
};
use tokio::sync::RwLock;

async fn create_test_app() -> (axum::Router, sqlx::PgPool) {
    let db = crate::test_db::get_test_database().await;
    let config = AppConfig::for_testing();

    let redis_env = crate::test_db::get_test_environment().await;
    let redis_client =
        Arc::new(redis::Client::open(redis_env.redis_uri()).expect("valid test redis URL"));

    let rate_limiter = Arc::new(RateLimiter::new(redis_client.clone()));
    let deny_list = Arc::new(DenyList::new(redis_client.clone()));
    let session_cache = Arc::new(SessionCache::new(redis_client.clone(), 300));
    let gps_history = Arc::new(GpsHistoryService::new(redis_client.clone()));
    // Every oneshot()-driven request in every test binary has no real
    // ConnectInfo, so the rate limiter's client-IP fallback ("unknown-peer")
    // is the SAME key across all of them — and since the shared testcontainers
    // Redis instance is now one instance reused by every test binary (not a
    // fresh one per binary), that one bucket is contended by the whole suite
    // running concurrently. The production default (3 registrations/window)
    // would trip spuriously under that load; raise it here, in this test-only
    // config, not in SystemConfig::default() itself.
    let mut default_config = SystemConfig::default();
    default_config.rate_limits.registration_max_requests = 100_000;
    default_config.rate_limits.login_max_requests = 100_000;
    let system_config = Arc::new(RwLock::new(default_config));

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
        db: db.clone(),
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

    (routes::create_routes(state), db)
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(SET_COOKIE).iter().find_map(|v| {
        let s = v.to_str().ok()?;
        let (kv, _rest) = s.split_once(';').unwrap_or((s, ""));
        let (k, val) = kv.split_once('=')?;
        (k == name).then(|| val.to_string())
    })
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}))
}

/// A logged-in test client: holds the auth + CSRF cookies and builds
/// authenticated requests the way the real admin/mentor frontends do.
struct Client {
    admin_token: String,
    csrf_token: String,
}

impl Client {
    /// Bootstraps or logs in, then makes one GET to pick up a fresh CSRF
    /// cookie (mirrors the real frontend: any page load re-issues one).
    async fn login(app: &axum::Router, username: &str, password: &str) -> Self {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/admin/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &serde_json::json!({ "username": username, "password": password }),
                        )
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "login should succeed");
        let admin_token =
            extract_cookie(response.headers(), "admin_token").expect("login sets admin_token");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/profile")
                    .header("cookie", format!("admin_token={}", admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let csrf_token =
            extract_cookie(response.headers(), "csrf_token").expect("GET issues csrf_token");

        Self {
            admin_token,
            csrf_token,
        }
    }

    fn cookie_header(&self) -> String {
        format!(
            "admin_token={}; csrf_token={}",
            self.admin_token, self.csrf_token
        )
    }

    async fn get(&self, app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, json_body(response).await)
    }

    async fn mutate(
        &self,
        app: &axum::Router,
        method: &str,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .header("x-csrf-token", &self.csrf_token)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, json_body(response).await)
    }
}

/// Registers via the public bootstrap endpoint, bypassing the client-side
/// CSRF dance since `/api/admin/register` is public (no csrf_middleware).
async fn register(
    app: &axum::Router,
    username: &str,
    email: &str,
) -> (StatusCode, serde_json::Value) {
    let config = AppConfig::for_testing();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "username": username,
                        "email": email,
                        "password": "correct-horse-battery-staple",
                        "adminSecret": config.admin_secret,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

/// Usernames are capped at 30 chars (see `create_admin_user`'s validation),
/// so this keeps well under that while still being unique per test run.
fn unique(prefix: &str) -> String {
    format!(
        "{}-{}",
        prefix,
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

/// Seeds an admin row directly via SQL, bypassing `/api/admin/register`
/// entirely. Every test below except the bootstrap-lockdown test itself uses
/// this instead of the public register endpoint: the lockdown check ("does
/// *any* super_admin exist") is a global invariant over the whole shared
/// test database, so only one test can safely exercise that endpoint's
/// bootstrap path without racing every other test that also wants a
/// super_admin to log in as (see `register_bootstraps_super_admin_then_locks_itself`).
async fn seed_admin(
    pool: &sqlx::PgPool,
    username: &str,
    email: &str,
    password: &str,
    role: &str,
) -> uuid::Uuid {
    let hashed = Admin::hash_password(password).unwrap();
    sqlx::query_scalar(
        "INSERT INTO admins (id, username, email, password, role, is_active, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, true, 0, now()) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(username)
    .bind(email)
    .bind(hashed)
    .bind(role)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The core security-relevant behaviour of the bootstrap lockdown: the first
/// registration mints a super_admin, and every registration attempt after
/// that — even with a perfectly valid ADMIN_SECRET — is a 404, because
/// account creation moves exclusively to the authenticated User Management
/// panel once a super_admin exists.
///
/// `#[file_serial(admin_bootstrap)]` (shared with every other test in this file):
/// this test's assertion that the *first* registration becomes super_admin
/// is a race against any other test in this file inserting its own
/// super_admin — serializing the whole group, plus the explicit DELETE
/// below, makes this test's view of "no super_admin exists yet" reliable
/// regardless of execution order.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn register_bootstraps_super_admin_then_locks_itself() {
    let (app, db) = create_test_app().await;
    // The other tests in this `#[file_serial(admin_bootstrap)]` group each seed
    // their own super_admin (plus locations/batches/sessions/audit-log rows
    // that FK-reference it) before this test gets its turn. TRUNCATE...
    // CASCADE clears all of it in one statement — safe here specifically
    // because serialization guarantees no other test in the group is
    // running concurrently, and every other test's assertions have already
    // completed by the time this one starts.
    sqlx::query(
        "TRUNCATE TABLE admins, locations, batches, students, sessions, attendances, \
         admin_audit_log RESTART IDENTITY CASCADE",
    )
    .execute(&db)
    .await
    .unwrap();

    let user1 = unique("bootstrap-admin");
    let (status, body) = register(&app, &user1, &format!("{}@example.com", user1)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "first registration should succeed: {body:?}"
    );
    assert_eq!(body["admin"]["role"], "super_admin");

    let user2 = unique("second-admin");
    let (status, _body) = register(&app, &user2, &format!("{}@example.com", user2)).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "registration must be closed once a super_admin exists"
    );
}

/// A super-admin creates a mentor via User Management; the mentor cannot
/// reach `/api/admin/users` (super-admin-only) or create sessions (also
/// super-admin-only) even while authenticated.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn super_admin_creates_mentor_and_role_gates_are_enforced() {
    let (app, db) = create_test_app().await;

    let super_username = unique("super");
    seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor_username = unique("mentor");
    let (status, body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor_username,
                "email": format!("{}@example.com", mentor_username),
                "password": "mentor-password-123",
                "fullName": "Test Mentor",
                "collegeName": "XYZ College",
                "role": "admin",
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "super-admin should be able to create a mentor: {body:?}"
    );
    assert_eq!(body["role"], "admin");
    assert_eq!(body["isActive"], true);

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;

    let (status, _) = mentor_client.get(&app, "/api/admin/users").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "mentor must not reach User Management"
    );

    // A well-formed body, so this actually exercises the role check inside
    // the handler rather than axum's body-extraction rejection (an empty
    // body 422s before `create_session` ever runs).
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({ "locationId": uuid::Uuid::new_v4().to_string(), "durationMinutes": 30 }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "mentor must not be able to create sessions"
    );
}

/// A super-admin can edit their own profile fields, but cannot demote or
/// deactivate themselves — there's no email-based recovery to undo it.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn super_admin_cannot_change_own_role_or_active_status() {
    let (app, db) = create_test_app().await;

    let username = unique("self-guard");
    let self_id = seed_admin(
        &db,
        &username,
        &format!("{}@example.com", username),
        "super-password-123",
        "super_admin",
    )
    .await
    .to_string();
    let client = Client::login(&app, &username, "super-password-123").await;

    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{self_id}"),
            serde_json::json!({ "isActive": false }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "must not be able to deactivate self"
    );

    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{self_id}"),
            serde_json::json!({ "role": "admin" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "must not be able to change own role"
    );

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{self_id}"),
            serde_json::json!({ "fullName": "Updated Name" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "non-guarded self-edits should still work: {body:?}"
    );
    assert_eq!(body["fullName"], "Updated Name");
}

/// Seeds a location + batch + one student directly via SQL (the roster/
/// manual-mark endpoints only care about table contents, not how a batch
/// was created — going through the multipart Excel-upload endpoint would
/// test unrelated code).
async fn seed_location_and_batch(
    pool: &sqlx::PgPool,
    created_by: uuid::Uuid,
    roll_number: &str,
) -> (uuid::Uuid, uuid::Uuid) {
    let location_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4())
    .bind("Test Hall")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap();

    let batch_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO batches (id, name, created_by) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4())
    .bind("Test Batch")
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO students (id, batch_id, position, name, roll_number) VALUES ($1, $2, 0, $3, $4)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(batch_id)
    .bind("Test Student")
    .bind(roll_number)
    .execute(pool)
    .await
    .unwrap();

    (location_id, batch_id)
}

/// Full mentor round trip: mark present, see it reflected in the roster
/// summary, then undo it back to unmarked. Exercises the mandatory-batch
/// exam-session creation path, role-scoped session visibility (the mentor
/// only sees this because it's assigned to them), and both new mutating
/// endpoints together.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn manual_attendance_mark_then_undo_roundtrip() {
    let (app, db) = create_test_app().await;

    let super_username = unique("exam-super");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor_username = unique("exam-mentor");
    let (_status, mentor_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor_username,
                "email": format!("{}@example.com", mentor_username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor_id = mentor_body["_id"].as_str().unwrap().to_string();

    let roll_number = "EXAM001";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "collegeName": "XYZ College",
                "startsAt": "2026-08-14T09:00:00Z",
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "exam session creation should succeed: {session_body:?}"
    );
    let session_id = session_body["_id"].as_str().unwrap().to_string();

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;

    let (status, roster) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "mentor should see their assigned session's roster: {roster:?}"
    );
    assert_eq!(roster["summary"]["unmarked"], 1);
    assert_eq!(roster["students"][0]["status"], "unmarked");

    let (status, mark_body) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "present" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "manual mark should succeed: {mark_body:?}"
    );
    assert_eq!(mark_body["status"], "present");

    let (_, roster_after_mark) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(roster_after_mark["summary"]["present"], 1);
    assert_eq!(roster_after_mark["summary"]["unmarked"], 0);

    let (status, _) = mentor_client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/sessions/{session_id}/attendance/manual/{roll_number}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "undo should succeed");

    let (_, roster_after_undo) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        roster_after_undo["summary"]["unmarked"], 1,
        "undo should revert to unmarked"
    );
}

/// A session scheduled for the future must reject manual marking outright —
/// the mentor frontend has its own "starts soon" gate, but that's a UI
/// convenience only, so the API must enforce this independently of any
/// client.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn manual_attendance_rejects_marking_before_session_starts() {
    let (app, db) = create_test_app().await;

    let super_username = unique("exam-super");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor_username = unique("exam-mentor");
    let (_status, mentor_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor_username,
                "email": format!("{}@example.com", mentor_username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor_id = mentor_body["_id"].as_str().unwrap().to_string();

    let roll_number = "FUTURE001";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let future_starts_at = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "collegeName": "XYZ College",
                "startsAt": future_starts_at,
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "exam session creation should succeed: {session_body:?}"
    );
    let session_id = session_body["_id"].as_str().unwrap().to_string();

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;

    let (status, mark_body) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "present" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "marking a not-yet-started session must be rejected: {mark_body:?}"
    );

    let (_, roster) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        roster["summary"]["unmarked"], 1,
        "the rejected mark must not have taken effect"
    );
}

/// A student who self-submitted attendance for a roll number must never
/// have their forensic data silently overwritten by a mentor's manual mark
/// — the endpoint must 409 instead.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn manual_attendance_conflicts_with_self_submitted_row() {
    let (app, db) = create_test_app().await;

    let super_username = unique("conflict-super");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor_username = unique("conflict-mentor");
    let (_status, mentor_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor_username,
                "email": format!("{}@example.com", mentor_username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor_id = mentor_body["_id"].as_str().unwrap().to_string();

    let roll_number = "CONFLICT001";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (_status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "collegeName": "XYZ College",
                "startsAt": "2026-08-14T09:00:00Z",
                "durationMinutes": 60,
            }),
        )
        .await;
    let session_id: uuid::Uuid = session_body["_id"].as_str().unwrap().parse().unwrap();

    // Seed a self-submitted row directly (mirrors the minimal set of
    // NOT-NULL columns submit_attendance would populate).
    sqlx::query(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, \
            source, status, captured_at \
         ) VALUES ($1, $2, $3, $4, 'https://example.com/p.jpg', 'photos/p', $5, $6, 0.0, true, \
            'self_submitted', 'present', now())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(session_id)
    .bind("Test Student")
    .bind(roll_number)
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .execute(&db)
    .await
    .unwrap();

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;
    let (status, body) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "absent" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "manual mark must not overwrite a self-submitted row: {body:?}"
    );
}

/// An exam session can be assigned to more than one mentor and needs no
/// location at all (manual attendance isn't geofenced) — both are new
/// behaviour on top of the original single-mentor, location-mandatory shape.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn exam_session_supports_multiple_mentors_and_no_location() {
    let (app, db) = create_test_app().await;

    let super_username = unique("multi-super");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor1_username = unique("multi-mentor-a");
    let (_status, mentor1_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor1_username,
                "email": format!("{}@example.com", mentor1_username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor1_id = mentor1_body["_id"].as_str().unwrap().to_string();

    let mentor2_username = unique("multi-mentor-b");
    let (_status, mentor2_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor2_username,
                "email": format!("{}@example.com", mentor2_username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor2_id = mentor2_body["_id"].as_str().unwrap().to_string();

    // seed_location_and_batch always creates a Location row too, but the
    // create-session payload below deliberately omits locationId — an exam
    // session doesn't use one.
    let roll_number = "MULTI001";
    let (_unused_location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor1_id, mentor2_id],
                "collegeName": "Multi-Mentor College",
                "startsAt": "2026-08-14T09:00:00Z",
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "exam session with no location should succeed: {session_body:?}"
    );
    assert!(
        session_body["locationId"].is_null(),
        "exam session should have no location"
    );

    let returned_ids: std::collections::HashSet<String> = session_body["assignedAdminIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        returned_ids,
        std::collections::HashSet::from([mentor1_id.clone(), mentor2_id.clone()])
    );
    assert_eq!(
        session_body["assignedAdminNames"].as_array().unwrap().len(),
        2
    );

    let session_id = session_body["_id"].as_str().unwrap().to_string();

    // Both assigned mentors can see the roster...
    let mentor1_client = Client::login(&app, &mentor1_username, "mentor-password-123").await;
    let (status, _) = mentor1_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "first assigned mentor should see the roster"
    );

    let mentor2_client = Client::login(&app, &mentor2_username, "mentor-password-123").await;
    let (status, _) = mentor2_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "second assigned mentor should see the roster"
    );

    // ...but an unassigned mentor cannot.
    let outsider_username = unique("multi-outsider");
    let (_status, _) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": outsider_username,
                "email": format!("{}@example.com", outsider_username),
                "password": "outsider-password-123",
                "role": "admin",
            }),
        )
        .await;
    let outsider_client = Client::login(&app, &outsider_username, "outsider-password-123").await;
    let (status, _) = outsider_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unassigned mentor must not see this session"
    );
}
