//! Regression coverage for `POST /api/s/{shortCode}/extension/events` — the
//! Phase 2 batched telemetry-ingestion endpoint. No pairing flow exists yet
//! (that's Phase 3), so these tests simulate it by inserting a
//! `session_device_locks` row directly, the same way a real pairing flow
//! will once it exists.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serial_test::file_serial;
use tower::ServiceExt;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Telemetry Ingestion Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Creates a session with an auto-generated short link, returning the code.
async fn create_session_with_short_code(
    app: &axum::Router,
    client: &Client,
    db: &sqlx::PgPool,
    admin_id: Uuid,
) -> String {
    let location_id = seed_location(db, admin_id).await;

    let (status, body) = client
        .mutate(
            app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "durationMinutes": 30,
                "shortlinkMode": "auto",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    body["shortCode"].as_str().unwrap().to_string()
}

async fn insert_active_lock(
    db: &sqlx::PgPool,
    session_id: Uuid,
    roll_number: &str,
    extension_instance_id: Uuid,
) {
    sqlx::query(
        "INSERT INTO session_device_locks \
         (id, session_id, roll_number, extension_instance_id, locked_at, status) \
         VALUES ($1, $2, $3, $4, now(), 'active')",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll_number)
    .bind(extension_instance_id)
    .execute(db)
    .await
    .unwrap();
}

async fn post_events(
    app: &axum::Router,
    short_code: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/s/{short_code}/extension/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

fn sample_batch(extension_instance_id: Uuid, roll_number: &str) -> serde_json::Value {
    serde_json::json!({
        "extensionInstanceId": extension_instance_id.to_string(),
        "rollNumber": roll_number,
        "events": [
            { "eventType": "heartbeat", "eventData": {}, "recordedAt": Utc::now().to_rfc3339() },
            { "eventType": "tab_visibility", "eventData": { "visible": true }, "recordedAt": Utc::now().to_rfc3339() },
        ],
    })
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn accepts_a_batch_matching_the_active_device_lock_and_writes_to_timescale() {
    let (app, db) = create_test_app().await;
    let timescale_db = crate::test_db::get_test_timescale_database().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    insert_active_lock(&db, session_id, "ROLL001", extension_instance_id).await;

    let (status, body) = post_events(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "roll001"),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "response body: {body:?}");
    assert_eq!(body["accepted"], serde_json::json!(2));

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM telemetry_events WHERE session_id = $1 AND roll_number = 'ROLL001'",
    )
    .bind(session_id)
    .fetch_one(&timescale_db)
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "both batched events should have been written to the telemetry_events hypertable"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_a_batch_with_no_active_device_lock() {
    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (status, body) =
        post_events(&app, &short_code, sample_batch(Uuid::new_v4(), "ROLL002")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "no lock exists for this student yet: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_a_batch_from_a_device_that_is_not_the_locked_one() {
    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let locked_device = Uuid::new_v4();
    insert_active_lock(&db, session_id, "ROLL003", locked_device).await;

    // A different device claiming to be the same student.
    let other_device = Uuid::new_v4();
    let (status, body) =
        post_events(&app, &short_code, sample_batch(other_device, "ROLL003")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a mismatched device must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_an_empty_batch() {
    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (status, body) = post_events(
        &app,
        &short_code,
        serde_json::json!({
            "extensionInstanceId": Uuid::new_v4().to_string(),
            "rollNumber": "ROLL004",
            "events": [],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}

// ===================================================================
// Signed telemetry token auth path (transitional dual-mode — see
// controllers::telemetry::ingest_telemetry's doc comment). These insert a
// lock WITH `telemetry_token_jti`/`telemetry_token_expires_at` set directly,
// the same "seed the row a real flow would produce" technique the plaintext
// tests above use for `session_device_locks` itself.
// ===================================================================

async fn insert_active_lock_with_token(
    db: &sqlx::PgPool,
    session_id: Uuid,
    roll_number: &str,
    extension_instance_id: Uuid,
    jti: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO session_device_locks \
         (id, session_id, roll_number, extension_instance_id, locked_at, status, telemetry_token_jti, telemetry_token_expires_at) \
         VALUES ($1, $2, $3, $4, now(), 'active', $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll_number)
    .bind(extension_instance_id)
    .bind(jti)
    .bind(expires_at)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn post_events_with_bearer(
    app: &axum::Router,
    short_code: &str,
    body: serde_json::Value,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/s/{short_code}/extension/events"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({})),
    )
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn accepts_a_batch_authenticated_with_a_valid_signed_token() {
    use attendance_geotag_backend::{config::AppConfig, models::generate_telemetry_token};

    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    let config = AppConfig::for_testing();
    // A signed token needs a lock_id to bind to, so a two-step seed: reserve
    // the lock row's id first, then generate the token, then fill it in —
    // mirrors how finish_pairing generates new_lock_id before the INSERT.
    let lock_id = Uuid::new_v4();
    let issued = generate_telemetry_token(
        session_id,
        "TELJWT001",
        extension_instance_id,
        lock_id,
        &config.jwt_secret,
    )
    .unwrap();
    sqlx::query(
        "INSERT INTO session_device_locks \
         (id, session_id, roll_number, extension_instance_id, locked_at, status, telemetry_token_jti, telemetry_token_expires_at) \
         VALUES ($1, $2, $3, $4, now(), 'active', $5, $6)",
    )
    .bind(lock_id)
    .bind(session_id)
    .bind("TELJWT001")
    .bind(extension_instance_id)
    .bind(&issued.jti)
    .bind(issued.expires_at)
    .execute(&db)
    .await
    .unwrap();

    let (status, body) = post_events_with_bearer(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "teljwt001"),
        &issued.token,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_a_signed_token_whose_jti_does_not_match_the_locks_current_token() {
    use attendance_geotag_backend::{config::AppConfig, models::generate_telemetry_token};

    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    let config = AppConfig::for_testing();
    let lock_id = Uuid::new_v4();

    // The token presented is well-formed and correctly signed, but the lock
    // row's stored jti is a DIFFERENT value (simulating a stale token from
    // before a refresh/rotation the client missed).
    let stale_token = generate_telemetry_token(
        session_id,
        "TELJWT002",
        extension_instance_id,
        lock_id,
        &config.jwt_secret,
    )
    .unwrap();
    insert_active_lock_with_token(
        &db,
        session_id,
        "TELJWT002",
        extension_instance_id,
        "a-completely-different-current-jti",
        Utc::now() + chrono::Duration::minutes(30),
    )
    .await;

    let (status, body) = post_events_with_bearer(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "teljwt002"),
        &stale_token.token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a token not matching the lock's current jti must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_an_expired_signed_token() {
    use attendance_geotag_backend::{config::AppConfig, models::TelemetryClaims};
    use jsonwebtoken::{encode, EncodingKey, Header};

    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    let lock_id = Uuid::new_v4();
    let config = AppConfig::for_testing();

    // Hand-crafted rather than via `generate_telemetry_token` (which always
    // uses the real ~30-minute lifetime) — this needs an `exp` already in
    // the past to exercise `jsonwebtoken::decode`'s own expiry check inside
    // `verify_telemetry_token`.
    let now = chrono::Utc::now().timestamp() as usize;
    let expired_claims = TelemetryClaims {
        session_id,
        roll_number: "TELEXP001".to_string(),
        extension_instance_id,
        lock_id,
        exp: now.saturating_sub(3600),
        iat: now.saturating_sub(7200),
        jti: Uuid::new_v4().to_string(),
    };
    let expired_token = encode(
        &Header::default(),
        &expired_claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .unwrap();

    insert_active_lock_with_token(
        &db,
        session_id,
        "TELEXP001",
        extension_instance_id,
        &expired_claims.jti,
        Utc::now() + chrono::Duration::minutes(30),
    )
    .await;

    let (status, body) = post_events_with_bearer(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "telexp001"),
        &expired_token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an expired telemetry token must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_a_tampered_signed_token() {
    use attendance_geotag_backend::{config::AppConfig, models::generate_telemetry_token};

    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    let lock_id = Uuid::new_v4();
    let config = AppConfig::for_testing();

    let issued = generate_telemetry_token(
        session_id,
        "TELTAMPER001",
        extension_instance_id,
        lock_id,
        &config.jwt_secret,
    )
    .unwrap();
    insert_active_lock_with_token(
        &db,
        session_id,
        "TELTAMPER001",
        extension_instance_id,
        &issued.jti,
        issued.expires_at,
    )
    .await;

    // Flip the last character of the signature — still well-formed
    // (three dot-separated base64 segments), but must fail verification.
    let mut tampered = issued.token.clone();
    let last = tampered.pop().unwrap();
    tampered.push(if last == 'a' { 'b' } else { 'a' });

    let (status, body) = post_events_with_bearer(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "teltamper001"),
        &tampered,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a tampered telemetry token must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn legacy_plaintext_path_still_works_when_no_authorization_header_is_sent() {
    // Regression guard for the transitional dual-mode rollout: an extension
    // that hasn't updated yet (sends no Authorization header at all) must
    // keep working exactly as before.
    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(&db)
            .await
            .unwrap();

    let extension_instance_id = Uuid::new_v4();
    // No telemetry_token_jti set at all — an old-style lock row.
    insert_active_lock(&db, session_id, "TELLEGACY001", extension_instance_id).await;

    let (status, body) = post_events(
        &app,
        &short_code,
        sample_batch(extension_instance_id, "tellegacy001"),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rejects_a_batch_over_the_max_size() {
    let (app, db) = create_test_app().await;
    let username = format!("telemetry-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let short_code = create_session_with_short_code(&app, &client, &db, admin_id).await;

    // Size check happens before the device-lock lookup (same as the empty-batch
    // case above), so no lock needs to be inserted for this to 400 rather than 403.
    let events: Vec<serde_json::Value> = (0..201)
        .map(|_| {
            serde_json::json!({
                "eventType": "heartbeat",
                "eventData": {},
                "recordedAt": Utc::now().to_rfc3339(),
            })
        })
        .collect();
    let (status, body) = post_events(
        &app,
        &short_code,
        serde_json::json!({
            "extensionInstanceId": Uuid::new_v4().to_string(),
            "rollNumber": "ROLL005",
            "events": events,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}
