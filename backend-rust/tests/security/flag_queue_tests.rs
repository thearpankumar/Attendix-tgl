//! Coverage for the unified flagged-attendance review queue
//! (`admin_security::get_flag_queue`/`bulk_review_flags`), which replaces the
//! old `controllers::admin::flags` module ("System A" in the audit) — that
//! endpoint only ever matched the legacy `device_flag` column and had no
//! pagination; this one surfaces every flag shape and paginates.

use axum::http::StatusCode;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Flag Queue Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_session(pool: &sqlx::PgPool, location_id: Uuid, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO sessions (id, location_id, expires_at, is_active, created_by, token_hash, token_prefix) \
         VALUES ($1, $2, now() + interval '1 hour', true, $3, $4, $5) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(location_id)
    .bind(created_by)
    .bind(format!("hash-{}", Uuid::new_v4()))
    .bind("testpfx")
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Minimal direct-SQL attendance seed — driving the full self-submission
/// flow (GPS + photo + anti-fraud middleware) just to get a flagged row on
/// disk would be a lot of unrelated scaffolding for what this test actually
/// needs to exercise: the *queue*, not the submission pipeline.
#[allow(clippy::too_many_arguments)]
async fn seed_attendance(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    roll_number: &str,
    flagged: bool,
    flag_reviewed: bool,
    device_flag: Option<&str>,
    flag_severity: Option<&str>,
    gps_anomalies: serde_json::Value,
    emulator_flags: serde_json::Value,
    integrity_checks: serde_json::Value,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, captured_at, \
            flagged, flag_reviewed, device_flag, flag_severity, \
            gps_anomalies, emulator_flags, integrity_checks \
         ) VALUES ( \
            $1, $2, $3, $4, '', '', $5, $6, 0.0, now(), \
            $7, $8, $9, $10, $11, $12, $13 \
         ) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(format!("Student {roll_number}"))
    .bind(roll_number)
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(flagged)
    .bind(flag_reviewed)
    .bind(device_flag)
    .bind(flag_severity)
    .bind(gps_anomalies)
    .bind(emulator_flags)
    .bind(integrity_checks)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn unique(prefix: &str) -> String {
    format!("{}-{}", prefix, &Uuid::new_v4().simple().to_string()[..8])
}

#[tokio::test]
async fn queue_surfaces_every_flag_shape_not_just_legacy_device_flag() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-admin");
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let location_id = seed_location(&db, admin_id).await;
    let session_id = seed_session(&db, location_id, admin_id).await;

    // Four flag shapes: legacy device_flag only, gps_anomalies only,
    // emulator_flags only, integrity_checks only — System A's `WHERE
    // device_flag IS NOT NULL` would have missed the last three entirely.
    let device_flag_only = seed_attendance(
        &db, session_id, "DF001", false, false,
        Some("GPS_ANOMALY_DETECTED"), None,
        serde_json::json!([]), serde_json::json!([]), serde_json::json!([]),
    ).await;
    let gps_only = seed_attendance(
        &db, session_id, "GPS001", true, false, None, Some("high"),
        serde_json::json!([{"type": "POSITION_JUMP", "severity": "high", "detectedAt": chrono::Utc::now()}]),
        serde_json::json!([]), serde_json::json!([]),
    ).await;
    let emulator_only = seed_attendance(
        &db, session_id, "EMU001", true, false, None, Some("medium"),
        serde_json::json!([]),
        serde_json::json!([{"type": "WEBGL_RENDERER_EMULATOR", "severity": "medium"}]),
        serde_json::json!([]),
    ).await;
    let integrity_only = seed_attendance(
        &db, session_id, "INT001", true, false, None, Some("low"),
        serde_json::json!([]), serde_json::json!([]),
        serde_json::json!([{"type": "TIMING_MANIPULATION", "passed": false}]),
    ).await;

    let (status, body) = client
        .get(&app, "/api/admin/security/flags/queue")
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");

    let ids: Vec<String> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["_id"].as_str().unwrap().to_string())
        .collect();

    for expected in [device_flag_only, gps_only, emulator_only, integrity_only] {
        assert!(
            ids.contains(&expected.to_string()),
            "queue must include attendance {expected} — got {ids:?}"
        );
    }
    assert!(body["total"].as_i64().unwrap() >= 4);
}

#[tokio::test]
async fn queue_without_session_id_requires_super_admin() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-mentor");
    seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "mentor",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let (status, _) = client.get(&app, "/api/admin/security/flags/queue").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn queue_supports_pagination_and_severity_filter() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-page");
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let location_id = seed_location(&db, admin_id).await;
    let session_id = seed_session(&db, location_id, admin_id).await;

    for i in 0..3 {
        seed_attendance(
            &db, session_id, &format!("PAGE{i:03}"), true, false, None, Some("high"),
            serde_json::json!([{"type": "POSITION_JUMP", "severity": "high", "detectedAt": chrono::Utc::now()}]),
            serde_json::json!([]), serde_json::json!([]),
        ).await;
    }

    let (status, body) = client
        .get(
            &app,
            &format!(
                "/api/admin/security/flags/queue?sessionId={session_id}&pageSize=2&page=1"
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"].as_i64().unwrap(), 3);
    assert_eq!(body["page"].as_i64().unwrap(), 1);
    assert_eq!(body["pageSize"].as_i64().unwrap(), 2);

    let (status, body) = client
        .get(
            &app,
            &format!("/api/admin/security/flags/queue?sessionId={session_id}&severity=low"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        0,
        "no seeded row has severity=low"
    );
}

#[tokio::test]
async fn review_without_notes_is_rejected() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-notes");
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let location_id = seed_location(&db, admin_id).await;
    let session_id = seed_session(&db, location_id, admin_id).await;
    let attendance_id = seed_attendance(
        &db, session_id, "NOTES001", true, false, None, Some("high"),
        serde_json::json!([{"type": "POSITION_JUMP", "severity": "high", "detectedAt": chrono::Utc::now()}]),
        serde_json::json!([]), serde_json::json!([]),
    ).await;

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/security/attendance/{attendance_id}/review"),
            serde_json::json!({ "action": "approve" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "review with no notes must be rejected: {body:?}"
    );

    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/security/attendance/{attendance_id}/review"),
            serde_json::json!({ "action": "approve", "notes": "Verified with student, false positive" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (flag_reviewed, review_notes): (bool, Option<String>) = sqlx::query_as(
        "SELECT flag_reviewed, review_notes FROM attendances WHERE id = $1",
    )
    .bind(attendance_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(flag_reviewed);
    assert_eq!(
        review_notes.as_deref(),
        Some("Verified with student, false positive")
    );
}

#[tokio::test]
async fn bulk_review_requires_notes_and_updates_every_row() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-bulk");
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let location_id = seed_location(&db, admin_id).await;
    let session_id = seed_session(&db, location_id, admin_id).await;

    let mut ids = vec![];
    for i in 0..3 {
        ids.push(
            seed_attendance(
                &db, session_id, &format!("BULK{i:03}"), true, false, None, Some("medium"),
                serde_json::json!([]),
                serde_json::json!([{"type": "WEBGL_RENDERER_EMULATOR", "severity": "medium"}]),
                serde_json::json!([]),
            )
            .await
            .to_string(),
        );
    }

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/security/flags/bulk-review",
            serde_json::json!({ "ids": ids, "action": "reject" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "notes required: {body:?}");

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/security/flags/bulk-review",
            serde_json::json!({ "ids": ids, "action": "reject", "notes": "Batch reviewed, all false positives from a known lab GPU" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");
    assert_eq!(body["updated"].as_i64().unwrap(), 3);

    let reviewed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND flag_reviewed = true",
    )
    .bind(session_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(reviewed_count, 3);
}

/// System A ("controllers::admin::flags::get_flagged_attendance"/
/// "review_attendance") is gone — its routes must not still respond.
#[tokio::test]
async fn legacy_system_a_routes_are_removed() {
    let (app, db) = create_test_app().await;
    let username = unique("flagqueue-legacy");
    seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let (status, _) = client.get(&app, "/api/admin/flagged").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/attendance/{}/review", Uuid::new_v4()),
            serde_json::json!({ "reviewed": true }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
