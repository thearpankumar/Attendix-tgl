//! Regression coverage for the Phase 3 device-pairing endpoints under
//! `/api/s/{shortCode}/extension/pair/...`.
//!
//! The full happy-path crypto ceremony (`finish_pairing_authentication`
//! actually verifying a real passkey assertion) is not simulated here — no
//! test in this codebase currently drives a full virtual-authenticator
//! WebAuthn ceremony end-to-end (see `public_webauthn.rs`'s own
//! `finish_authentication`, which has the same gap). What IS covered: the
//! pairing state machine (start -> pending -> completed -> locked), its
//! rejection paths that run before any crypto verification, and — the part
//! with the most real risk of a data-integrity bug — that `finish_pairing`
//! never leaves two `active` device locks for the same (session,
//! roll_number), superseding the prior one instead.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
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
    .bind("Extension Pairing Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_session_with_short_code(
    app: &axum::Router,
    client: &Client,
    db: &sqlx::PgPool,
    admin_id: Uuid,
) -> (String, Uuid) {
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
    let short_code = body["shortCode"].as_str().unwrap().to_string();
    let session_id: Uuid =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_one(db)
            .await
            .unwrap();
    (short_code, session_id)
}

async fn plain_post(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({})),
    )
}

async fn plain_get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
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

/// Syntactically well-formed but cryptographically meaningless — enough to
/// deserialize into `PublicKeyCredential` for tests that exercise a
/// rejection path evaluated before any signature verification happens.
fn bogus_credential() -> serde_json::Value {
    serde_json::json!({
        "id": "cred-id",
        "rawId": "e30=",
        "response": {
            "clientDataJSON": "e30=",
            "authenticatorData": "e30=",
            "signature": "e30=",
        },
        "type": "public-key",
    })
}

async fn setup(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let username = format!("pairing-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(app, &username, "correct-horse-battery-staple").await;
    (client, admin_id)
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn start_pairing_creates_a_pending_request() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let extension_instance_id = Uuid::new_v4();
    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": extension_instance_id.to_string() }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    let pairing_code = body["pairingCode"].as_str().unwrap().to_string();
    // Must be fully qualified (scheme + host), not a bare path — a phone's
    // camera app needs a real link to open from the QR code. AppConfig::for_testing()
    // sets public_base_url to "http://localhost".
    assert_eq!(
        body["pairingUrl"],
        serde_json::json!(format!(
            "http://localhost/attend/pair/{short_code}/{pairing_code}"
        ))
    );

    let (db_status, db_session_id, db_extension_instance_id): (String, Uuid, Uuid) = sqlx::query_as(
        "SELECT status, session_id, extension_instance_id FROM extension_pairing_requests WHERE pairing_code = $1",
    )
    .bind(&pairing_code)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(db_status, "pending");
    assert_eq!(db_session_id, session_id);
    assert_eq!(db_extension_instance_id, extension_instance_id);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn pairing_status_reports_pending_then_expired() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, _session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    let (status, body) = plain_get(
        &app,
        &format!("/api/s/{short_code}/extension/pair/status/{pairing_code}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], serde_json::json!("pending"));

    sqlx::query("UPDATE extension_pairing_requests SET expires_at = now() - interval '1 minute' WHERE pairing_code = $1")
        .bind(&pairing_code)
        .execute(&db)
        .await
        .unwrap();

    let (status, body) = plain_get(
        &app,
        &format!("/api/s/{short_code}/extension/pair/status/{pairing_code}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], serde_json::json!("expired"));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn pairing_status_for_unknown_code_404s() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, _session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (status, body) = plain_get(
        &app,
        &format!("/api/s/{short_code}/extension/pair/status/does-not-exist"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_rejects_a_code_from_another_session() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code_a, _session_a) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;
    let (short_code_b, _session_b) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code_a}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    // Session A's pairing code presented against session B's short code.
    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code_b}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": Uuid::new_v4().to_string(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_rejects_a_non_pending_code() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, _session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    sqlx::query("UPDATE extension_pairing_requests SET status = 'completed', roll_number = 'ROLLX', completed_at = now() WHERE pairing_code = $1")
        .bind(&pairing_code)
        .execute(&db)
        .await
        .unwrap();

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": Uuid::new_v4().to_string(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}

// =================== WebAuthn challenge correlation (challengeId) ===================
//
// `finish_pairing_authentication` and `finish_authentication` (attendance
// check-in) both resolve their ceremony's subject through the same shared
// `public_webauthn::resolve_authentication_subject` ->
// `take_pending_challenge` pair. The bug this section regression-tests: the
// old lookup picked the single most-recently-created, unused challenge row
// for a short_code + type with no per-ceremony correlation at all
// (`ORDER BY created_at DESC LIMIT 1`), so two ceremonies pending at once on
// the same short_code (e.g. two students authenticating near-simultaneously)
// could have the second-to-finish one consume the OTHER ceremony's
// challenge. The fix threads the `start_*` endpoint's own challenge row id
// (`challengeId`) back through `finish_*` for a direct primary-key lookup.
//
// These insert challenge rows directly via SQL — the same technique this
// file already uses for `extension_pairing_requests` — rather than driving a
// full WebAuthn ceremony, which (per this file's top-of-file comment) no
// test in this codebase currently simulates end-to-end. A bogus credential
// cannot pass real signature verification, so every case below is provable
// only up to (and including) the point `take_pending_challenge` runs: the
// meaningful, observable signal is which challenge row got marked `used`,
// not the final HTTP status past that point.

/// Inserts a pending authentication challenge row directly, bypassing
/// `start_authentication` (which requires a real enrolled credential and a
/// live webauthn-rs ceremony state we can't easily fabricate). `state` is
/// set to an empty JSON object — enough to satisfy `take_pending_challenge`'s
/// `state IS NOT NULL` predicate; nothing in the path up to and including
/// that function ever deserializes it.
async fn insert_pending_authentication_challenge(
    db: &sqlx::PgPool,
    challenge_id: Uuid,
    session_id: Uuid,
    short_code: &str,
    student_id: &str,
) {
    sqlx::query(
        "INSERT INTO webauthn_challenges \
         (id, student_id, challenge, challenge_type, session_id, short_code, student_name, expires_at, used, state, created_at) \
         VALUES ($1, $2, 'test-challenge', 'authentication', $3, $4, 'Test Student', now() + interval '5 minutes', false, '{}'::jsonb, now())",
    )
    .bind(challenge_id)
    .bind(student_id)
    .bind(session_id)
    .bind(short_code.to_lowercase())
    .execute(db)
    .await
    .unwrap();
}

async fn challenge_used(db: &sqlx::PgPool, challenge_id: Uuid) -> bool {
    sqlx::query_scalar("SELECT used FROM webauthn_challenges WHERE id = $1")
        .bind(challenge_id)
        .fetch_one(db)
        .await
        .unwrap()
}

/// The regression test that matters most: two ceremonies pending at once for
/// the same short_code. Finishing with the OLDER challenge's id must consume
/// exactly that row, never the newer one — under the old "most recent"
/// lookup, `ORDER BY created_at DESC LIMIT 1` would have picked the newer
/// row regardless of which id this call named, silently resolving to the
/// wrong student.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_consumes_only_the_named_challenge_not_the_most_recent() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let older_challenge_id = Uuid::new_v4();
    insert_pending_authentication_challenge(
        &db,
        older_challenge_id,
        session_id,
        &short_code,
        "STU-OLDER",
    )
    .await;
    // Created strictly after the row above, so a naive "most recent" lookup
    // would prefer this one over the id the request actually names below.
    let newer_challenge_id = Uuid::new_v4();
    insert_pending_authentication_challenge(
        &db,
        newer_challenge_id,
        session_id,
        &short_code,
        "STU-NEWER",
    )
    .await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    // Explicitly name the OLDER (not most-recently-created) challenge.
    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": older_challenge_id.to_string(),
        }),
    )
    .await;
    // A bogus credential can't pass real crypto verification, so this still
    // fails downstream — but NOT at the challenge-lookup stage, proving the
    // named id was found and matched.
    assert_ne!(
        status,
        StatusCode::BAD_REQUEST,
        "must get past challenge lookup (found via its own id), even though the crypto \
         step then fails on a bogus credential: {body:?}"
    );

    assert!(
        challenge_used(&db, older_challenge_id).await,
        "the explicitly named challenge must be consumed"
    );
    assert!(
        !challenge_used(&db, newer_challenge_id).await,
        "a different pending ceremony's challenge must be left untouched, even though it \
         was created more recently — this is exactly what the old 'most recent' lookup got \
         wrong"
    );
}

/// A `challengeId` for a challenge that has already been consumed (single-use
/// enforcement) must be rejected, not silently re-accepted.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_rejects_an_already_used_challenge_id() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let challenge_id = Uuid::new_v4();
    insert_pending_authentication_challenge(&db, challenge_id, session_id, &short_code, "STU-USED")
        .await;
    sqlx::query("UPDATE webauthn_challenges SET used = true WHERE id = $1")
        .bind(challenge_id)
        .execute(&db)
        .await
        .unwrap();

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": challenge_id.to_string(),
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an already-used challengeId must be rejected: {body:?}"
    );
}

/// A `challengeId` naming a challenge that belongs to a DIFFERENT short_code
/// must be rejected — it must not be resolvable by presenting it against an
/// unrelated session's short code.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_rejects_a_challenge_id_from_another_short_code() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code_a, session_a) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;
    let (short_code_b, _session_b) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    // Challenge belongs to session A's short code.
    let challenge_id = Uuid::new_v4();
    insert_pending_authentication_challenge(
        &db,
        challenge_id,
        session_a,
        &short_code_a,
        "STU-FOREIGN",
    )
    .await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code_b}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    // Presented against session B's short code.
    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code_b}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": challenge_id.to_string(),
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a challengeId from another short_code must be rejected: {body:?}"
    );
    assert!(
        !challenge_used(&db, challenge_id).await,
        "the foreign challenge must not be consumed by a mismatched short_code"
    );
}

/// A `challengeId` that was never inserted at all must be rejected cleanly,
/// not panic or 500.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_rejects_a_nonexistent_challenge_id() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, _session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/authenticate/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "credential": bogus_credential(),
            "challengeId": Uuid::new_v4().to_string(),
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a challengeId that was never issued must be rejected: {body:?}"
    );
}

/// Proves the atomicity claim in `take_pending_challenge`'s doc comment: two
/// concurrent finishes naming the SAME challenge_id (a legitimate
/// double-finish/retry race, not the cross-ceremony bug the tests above
/// cover) must have exactly one winner. A plain
/// `UPDATE ... WHERE id = $1 AND used = false ... RETURNING *` is claimed to
/// already guarantee this without `FOR UPDATE SKIP LOCKED`: Postgres locks
/// the row for whichever transaction reaches it first; the second blocks
/// until the first commits, then re-evaluates `used = false` against the
/// now-committed row and matches zero rows. This test drives that race for
/// real via `tokio::join!` rather than asserting it from reasoning alone.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_authentication_concurrent_double_finish_has_exactly_one_winner() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let challenge_id = Uuid::new_v4();
    insert_pending_authentication_challenge(&db, challenge_id, session_id, &short_code, "STU-RACE")
        .await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    let uri = format!("/api/s/{short_code}/extension/pair/authenticate/finish");
    let body = serde_json::json!({
        "pairingCode": pairing_code,
        "credential": bogus_credential(),
        "challengeId": challenge_id.to_string(),
    });

    // Both requests name the identical challenge_id and run concurrently
    // against the same app/pool.
    let (r1, r2) = tokio::join!(
        plain_post(&app, &uri, body.clone()),
        plain_post(&app, &uri, body.clone()),
    );
    let statuses = [r1.0, r2.0];

    let winners = statuses
        .iter()
        .filter(|s| **s != StatusCode::BAD_REQUEST)
        .count();
    let losers = statuses
        .iter()
        .filter(|s| **s == StatusCode::BAD_REQUEST)
        .count();

    assert_eq!(
        winners, 1,
        "exactly one concurrent finish must win (get past the challenge lookup): {statuses:?} \
         (bodies: {:?}, {:?})",
        r1.1, r2.1
    );
    assert_eq!(
        losers, 1,
        "exactly one concurrent finish must lose with 'no valid challenge found': {statuses:?}"
    );
    assert!(
        challenge_used(&db, challenge_id).await,
        "the challenge must end up consumed regardless of which request won"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_rejects_when_authentication_has_not_completed_yet() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, _session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let (_status, start_body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/start"),
        serde_json::json!({ "extensionInstanceId": Uuid::new_v4().to_string() }),
    )
    .await;
    let pairing_code = start_body["pairingCode"].as_str().unwrap().to_string();

    // Still 'pending' -- the extension polled too early.
    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        serde_json::json!({ "pairingCode": pairing_code }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}

/// Directly drives the state `finish_pairing_authentication` would have left
/// behind (a 'completed' pairing_requests row) to exercise `finish_pairing`
/// in isolation from the crypto ceremony — the same technique the Phase 2
/// telemetry tests use for `session_device_locks`.
async fn complete_pairing_request(
    db: &sqlx::PgPool,
    session_id: Uuid,
    extension_instance_id: Uuid,
    roll_number: &str,
) -> String {
    let pairing_code: String = sqlx::query_scalar(
        "INSERT INTO extension_pairing_requests \
         (id, session_id, extension_instance_id, pairing_code, status, roll_number, created_at, expires_at, completed_at) \
         VALUES ($1, $2, $3, $4, 'completed', $5, now(), now() + interval '5 minutes', now()) \
         RETURNING pairing_code",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(extension_instance_id)
    .bind(format!("test-{}", Uuid::new_v4().simple()))
    .bind(roll_number)
    .fetch_one(db)
    .await
    .unwrap();
    pairing_code
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_creates_the_lock_and_installation_row() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let extension_instance_id = Uuid::new_v4();
    let pairing_code =
        complete_pairing_request(&db, session_id, extension_instance_id, "ROLL001").await;

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        serde_json::json!({
            "pairingCode": pairing_code,
            "deviceFingerprintHash": "abc123",
            "browser": "chrome",
            "os": "windows",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(body["rollNumber"], serde_json::json!("ROLL001"));

    let (lock_status, lock_extension_id): (String, Uuid) = sqlx::query_as(
        "SELECT status, extension_instance_id FROM session_device_locks WHERE session_id = $1 AND roll_number = 'ROLL001'",
    )
    .bind(session_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(lock_status, "active");
    assert_eq!(lock_extension_id, extension_instance_id);

    let installation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM extension_installations WHERE extension_instance_id = $1",
    )
    .bind(extension_instance_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(installation_count, 1);
}

/// A retried `finish` call for a pairing_code that already succeeded (e.g.
/// the extension's popup got a network error on the first attempt's
/// response but the write actually landed) must be a no-op, not create a
/// second lock that supersedes the one it just created for itself.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_is_idempotent_on_retry() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let extension_instance_id = Uuid::new_v4();
    let pairing_code =
        complete_pairing_request(&db, session_id, extension_instance_id, "ROLL006").await;

    let request_body = serde_json::json!({
        "pairingCode": pairing_code,
        "deviceFingerprintHash": "abc123",
        "browser": "chrome",
        "os": "windows",
    });

    let (status1, body1) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        request_body.clone(),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED, "response body: {body1:?}");

    let (status2, body2) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        request_body,
    )
    .await;
    assert_eq!(
        status2,
        StatusCode::CREATED,
        "a retry must still report success: {body2:?}"
    );

    let lock_rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT status, extension_instance_id FROM session_device_locks WHERE session_id = $1 AND roll_number = 'ROLL006'",
    )
    .bind(session_id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        lock_rows.len(),
        1,
        "the retry must not create a second lock row: {lock_rows:?}"
    );
    assert_eq!(lock_rows[0].0, "active");
    assert_eq!(lock_rows[0].1, extension_instance_id);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_supersedes_a_prior_active_lock_instead_of_leaving_two_active() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let old_device = Uuid::new_v4();
    let old_lock_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO session_device_locks (id, session_id, roll_number, extension_instance_id, locked_at, status) \
         VALUES ($1, $2, 'ROLL002', $3, now(), 'active')",
    )
    .bind(old_lock_id)
    .bind(session_id)
    .bind(old_device)
    .execute(&db)
    .await
    .unwrap();

    let new_device = Uuid::new_v4();
    let pairing_code = complete_pairing_request(&db, session_id, new_device, "ROLL002").await;

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        serde_json::json!({ "pairingCode": pairing_code }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_device_locks WHERE session_id = $1 AND roll_number = 'ROLL002' AND status = 'active'",
    )
    .bind(session_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        active_count, 1,
        "exactly one active lock must remain, never two"
    );

    let (old_status, superseded_by): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, superseded_by FROM session_device_locks WHERE id = $1")
            .bind(old_lock_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(old_status, "superseded");
    assert!(
        superseded_by.is_some(),
        "the old lock must point at the row that replaced it"
    );

    let new_active: Uuid = sqlx::query_scalar(
        "SELECT id FROM session_device_locks WHERE session_id = $1 AND roll_number = 'ROLL002' AND status = 'active'",
    )
    .bind(session_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(superseded_by, Some(new_active));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn finish_pairing_rejects_an_expired_completed_request() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = setup(&app, &db).await;
    let (short_code, session_id) =
        create_session_with_short_code(&app, &client, &db, admin_id).await;

    let pairing_code = complete_pairing_request(&db, session_id, Uuid::new_v4(), "ROLL003").await;
    sqlx::query("UPDATE extension_pairing_requests SET expires_at = now() - interval '1 minute' WHERE pairing_code = $1")
        .bind(&pairing_code)
        .execute(&db)
        .await
        .unwrap();

    let (status, body) = plain_post(
        &app,
        &format!("/api/s/{short_code}/extension/pair/finish"),
        serde_json::json!({ "pairingCode": pairing_code }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body:?}");
}
