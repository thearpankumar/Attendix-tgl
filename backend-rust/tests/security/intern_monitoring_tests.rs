//! Regression coverage for Phase 5 of session monitoring: intern
//! continuous-monitoring reuses `recurring_session_rules`/`sessions`
//! directly via a new `sessionKind` field, rather than a parallel data
//! model — see migration 0008_intern_monitoring.sql and the design note in
//! `controllers::recurring_session_rule`. These tests confirm the
//! `intern_monitoring` kind works location-less/monitoring-forced-on, and
//! that ordinary `attendance` rules and sessions are completely unaffected
//! by making `location_id` nullable.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serial_test::file_serial;
use tower::ServiceExt;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

/// No `user-agent` header at all — `check_mobile` classifies this as a
/// non-mobile, non-masquerading desktop client, same as a real laptop
/// browser's UA would be. Used to exercise `resolution_mobile_check_middleware`
/// (`GET /session`'s desktop behavior differs by session kind — see its
/// doc comment) and to confirm every other scan-flow route stays strictly
/// mobile-only regardless.
async fn plain_get_as_desktop(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
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

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Intern Monitoring Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn login_super_admin(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let username = format!("intern-admin-{}", Uuid::new_v4().simple());
    let email = format!("{username}@example.com");
    let admin_id = seed_admin(
        db,
        &username,
        &email,
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(app, &username, "correct-horse-battery-staple").await;
    (client, admin_id)
}

fn intern_payload() -> serde_json::Value {
    serde_json::json!({
        "sessionKind": "intern_monitoring",
        "description": "Daily work-hours monitoring",
        "durationMinutes": 480,
        "runTimesLocal": ["09:00"],
        "timezone": "Asia/Kolkata",
        "daysOfWeek": [1, 2, 3, 4, 5],
        "monitoringEnabled": true,
        "classDurationMinutes": 480,
    })
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn creates_an_intern_monitoring_rule_with_no_location() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", intern_payload())
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert!(
        body["locationId"].is_null(),
        "expected no location: {body:?}"
    );
    assert_eq!(body["sessionKind"], serde_json::json!("intern_monitoring"));
    assert_eq!(body["monitoringEnabled"], serde_json::json!(true));
    assert_eq!(body["classDurationMinutes"], serde_json::json!(480));
    // A real locked short link is mandatory: the browser extension detects
    // which session to pair by matching a short code in the tab's URL, and
    // there is no other way to reach the pairing/registration flow — see
    // `controllers::recurring_session_rule::create_recurring_rule`'s
    // `is_intern_monitoring` shortlink-mode override.
    assert!(
        body["lockedShortCode"].is_string(),
        "an intern-monitoring rule must still get a real locked short link: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn intern_monitoring_rule_requires_monitoring_enabled() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let mut payload = intern_payload();
    payload["monitoringEnabled"] = serde_json::json!(false);

    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an intern monitoring rule without monitoring enabled must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn attendance_rule_still_requires_a_location() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    // No sessionKind -> defaults to "attendance", no locationId given.
    let payload = serde_json::json!({
        "description": "Missing location",
        "durationMinutes": 30,
        "runTimesLocal": ["09:00"],
        "timezone": "UTC",
        "daysOfWeek": [1, 2, 3, 4, 5],
    });

    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an attendance rule must still require a location: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn attendance_rule_with_a_location_is_unaffected_by_the_nullable_column() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let location_id = seed_location(&db, admin_id).await;

    let payload = serde_json::json!({
        "locationId": location_id.to_string(),
        "description": "Ordinary attendance rule",
        "durationMinutes": 30,
        "runTimesLocal": ["09:00"],
        "timezone": "UTC",
        "daysOfWeek": [1, 2, 3, 4, 5],
    });

    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(
        body["locationId"],
        serde_json::json!(location_id.to_string())
    );
    assert_eq!(body["sessionKind"], serde_json::json!("attendance"));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn creates_a_one_off_intern_monitoring_session_with_no_location() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "isInternMonitoring": true,
                "durationMinutes": 480,
                "classDurationMinutes": 480,
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert!(
        body["locationId"].is_null(),
        "expected no location: {body:?}"
    );
    assert_eq!(body["sessionKind"], serde_json::json!("intern_monitoring"));
    // Monitoring is forced on regardless of what the request sent — the
    // request above didn't even set monitoringEnabled.
    assert_eq!(body["monitoringEnabled"], serde_json::json!(true));
    assert_eq!(body["classDurationMinutes"], serde_json::json!(480));
    assert!(
        !body["monitoringEndsAt"].is_null(),
        "monitoringEndsAt must be set for an intern-monitoring session: {body:?}"
    );
    // A real auto-generated short link is mandatory here too — see
    // `controllers::session::create_session`'s `is_intern_monitoring`
    // shortlink-mode override, and the matching recurring-rule test above.
    let short_code = body["shortCode"]
        .as_str()
        .expect("an intern-monitoring session must get a real short link");

    // The exact bugs this closes: (1) `get_short_link_session` used to do an
    // unconditional location lookup and 404 with "Location not found" for
    // any location-less session — which every intern-monitoring session is;
    // (2) the route sat behind the strict, session-kind-blind
    // `mobile_check_middleware`, which 403s any non-mobile User-Agent —
    // meaning a desktop browser (exactly what the paired laptop uses) could
    // never load this page at all, so the extension had nothing to detect.
    // This is the public endpoint `StudentScan.tsx` calls on load, requested
    // here with NO mobile User-Agent to prove the desktop case specifically.
    let (status, session_body) =
        plain_get_as_desktop(&app, &format!("/api/s/{short_code}/session")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "opening an intern-monitoring session's short link from a desktop browser must not be blocked: {session_body:?}"
    );
    assert_eq!(session_body["valid"], serde_json::json!(true));
    assert_eq!(
        session_body["session"]["sessionKind"],
        serde_json::json!("intern_monitoring")
    );
    assert!(
        session_body["session"]["locationName"].is_null(),
        "expected no location name: {session_body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn intern_monitoring_session_honors_a_requested_custom_short_code() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let custom_code = format!("intern-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "isInternMonitoring": true,
                "durationMinutes": 480,
                "classDurationMinutes": 480,
                "shortlinkMode": "custom",
                "customShortCode": custom_code,
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(
        body["shortCode"],
        serde_json::json!(custom_code),
        "an intern session's admin-chosen custom code must be honored, not silently overridden with an auto-generated one: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn intern_monitoring_session_still_falls_back_to_auto_when_mode_is_none() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "isInternMonitoring": true,
                "durationMinutes": 480,
                "classDurationMinutes": 480,
                "shortlinkMode": "none",
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert!(
        body["shortCode"].is_string(),
        "an intern session explicitly requesting 'none' must still fall back to a real link, since the extension pairing flow depends on one: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn intern_monitoring_rule_honors_a_requested_custom_short_code() {
    let (app, db) = create_test_app().await;
    let (client, _admin_id) = login_super_admin(&app, &db).await;

    let custom_code = format!("intern-rule-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let mut payload = intern_payload();
    payload["shortlinkMode"] = serde_json::json!("custom");
    payload["customShortCode"] = serde_json::json!(custom_code);

    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(
        body["lockedShortCode"],
        serde_json::json!(custom_code),
        "an intern rule's admin-chosen custom code must be honored, not silently overridden with an auto-generated one: {body:?}"
    );
}

/// The desktop allowance above isn't just for `intern_monitoring` sessions —
/// an *ordinary* session with monitoring enabled needs it too, since pairing
/// the extension is still a "leave this page open on the laptop" flow (see
/// `resolution_mobile_check_middleware`'s doc comment). A plain, unmonitored
/// attendance session's info page must still reject a desktop browser
/// exactly as before, though — that's the boundary this and the next test
/// draw on either side of.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn ordinary_session_without_monitoring_still_rejects_a_desktop_browser_on_the_info_route() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let location_id = seed_location(&db, admin_id).await;

    let (status, body) = client
        .mutate(
            &app,
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
    let short_code = body["shortCode"].as_str().unwrap();

    let (status, session_body) =
        plain_get_as_desktop(&app, &format!("/api/s/{short_code}/session")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an ordinary, unmonitored session's info route must still reject a desktop browser: {session_body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn ordinary_session_with_monitoring_enabled_allows_a_desktop_browser_on_the_info_route() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let location_id = seed_location(&db, admin_id).await;

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "durationMinutes": 30,
                "shortlinkMode": "auto",
                "monitoringEnabled": true,
                "classDurationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    let short_code = body["shortCode"].as_str().unwrap();

    let (status, session_body) =
        plain_get_as_desktop(&app, &format!("/api/s/{short_code}/session")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a monitored ordinary session's info route must allow a desktop browser through to show pairing instructions: {session_body:?}"
    );
    assert_eq!(
        session_body["session"]["sessionKind"],
        serde_json::json!("attendance")
    );
    assert_eq!(
        session_body["session"]["monitoringEnabled"],
        serde_json::json!(true)
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn one_off_intern_monitoring_session_cannot_also_be_an_exam_session() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let batch_id: Uuid = sqlx::query_scalar(
        "INSERT INTO batches (id, name, created_by) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Intern/Exam Conflict Batch")
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "isInternMonitoring": true,
                "durationMinutes": 480,
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [admin_id.to_string()],
                "collegeName": "Example College",
                "startsAt": "2026-08-14T09:00:00Z",
            }),
        )
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a session cannot be both an exam session and an intern-monitoring session: {body:?}"
    );
}

/// Directly exercises the recurring scheduler (same technique as
/// `recurring_scheduler_tests.rs` / `session_monitoring_tests.rs`) to prove
/// an intern-monitoring rule generates a location-less, monitoring-enabled
/// session — not just that the rule row itself looks right.
#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn scheduler_generates_a_location_less_monitoring_enabled_occurrence() {
    let (_app, db) = create_test_app().await;
    let admin_id = seed_admin(
        &db,
        &format!("intern-sched-admin-{}", Uuid::new_v4().simple()),
        &format!("intern-sched-admin-{}@example.com", Uuid::new_v4().simple()),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;

    let rule_id = Uuid::new_v4();
    let past = chrono::Utc::now() - chrono::Duration::seconds(5);
    sqlx::query(
        "INSERT INTO recurring_session_rules \
         (id, location_id, batch_id, description, duration_minutes, run_times_local, timezone, \
          days_of_week, is_active, created_by, created_at, updated_at, locked_short_code, next_run_at, \
          monitoring_enabled, class_duration_minutes, session_kind) \
         VALUES ($1, NULL, NULL, 'intern monitoring scheduler test rule', 480, ARRAY['09:00'::time], 'UTC', \
                 ARRAY[1,2,3,4,5]::smallint[], true, $2, now(), now(), NULL, $3, true, 480, 'intern_monitoring')",
    )
    .bind(rule_id)
    .bind(admin_id)
    .bind(past)
    .execute(&db)
    .await
    .expect("insert due intern-monitoring recurring rule with no location");

    let claimed = attendance_geotag_backend::services::claim_and_generate_one(&db)
        .await
        .expect("claim pass should not error");
    assert!(claimed, "the due rule should have been claimed");

    let (location_id, monitoring_enabled, class_duration_minutes, session_kind): (
        Option<Uuid>,
        bool,
        Option<i32>,
        String,
    ) = sqlx::query_as(
        "SELECT location_id, monitoring_enabled, class_duration_minutes, session_kind \
         FROM sessions WHERE recurring_rule_id = $1",
    )
    .bind(rule_id)
    .fetch_one(&db)
    .await
    .expect("fetch generated session");

    assert!(
        location_id.is_none(),
        "generated session should have no location"
    );
    assert!(
        monitoring_enabled,
        "generated session should have monitoring enabled"
    );
    assert_eq!(class_duration_minutes, Some(480));
    assert_eq!(session_kind, "intern_monitoring");
}
