//! Regression coverage for the session-monitoring fields added in migration
//! 0005_session_monitoring: `monitoringEnabled` / `classDurationMinutes` /
//! `monitoringEndsAt` on both one-off sessions and recurring rules, the
//! validation that ties them together, propagation from a recurring rule
//! into its generated occurrences, and the same-day-only restriction on
//! `PATCH /api/admin/sessions/{id}/schedule`.

use axum::http::StatusCode;
use chrono::Utc;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Session Monitoring Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn login_super_admin(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let username = format!("mon-admin-{}", Uuid::new_v4().simple());
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

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn create_session_with_monitoring_enabled_persists_class_duration_and_monitoring_ends_at() {
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
                "monitoringEnabled": true,
                "classDurationMinutes": 90,
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(body["monitoringEnabled"], serde_json::json!(true));
    assert_eq!(body["classDurationMinutes"], serde_json::json!(90));
    assert!(
        !body["monitoringEndsAt"].is_null(),
        "monitoringEndsAt must be set when monitoring is enabled: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn create_session_with_monitoring_enabled_requires_class_duration() {
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
                "monitoringEnabled": true,
            }),
        )
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "monitoring on with no class duration must be rejected: {body:?}"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn create_session_without_monitoring_leaves_monitoring_fields_unset() {
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
            }),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    assert_eq!(body["monitoringEnabled"], serde_json::json!(false));
    assert!(body["classDurationMinutes"].is_null());
    assert!(body["monitoringEndsAt"].is_null());
}

/// Directly exercises the recurring scheduler (same technique as
/// `recurring_scheduler_tests.rs`) to prove a rule's `monitoringEnabled` /
/// `classDurationMinutes` land on the session it generates, not just on the
/// rule row itself.
///
/// `#[file_serial(recurring_scheduler)]`, not `admin_bootstrap`: this test
/// inserts a due row into the shared `recurring_session_rules` claim queue
/// and calls `claim_and_generate_one` directly, exactly like
/// `recurring_scheduler_tests.rs` does — without the same lock group, this
/// races with those tests' "claim the one due rule" assertions (each can
/// claim the other's due rule instead of its own).
#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn recurring_rule_monitoring_fields_propagate_to_generated_occurrence() {
    let (_app, db) = create_test_app().await;
    let admin_id = seed_admin(
        &db,
        &format!("mon-rrule-admin-{}", Uuid::new_v4().simple()),
        &format!("mon-rrule-admin-{}@example.com", Uuid::new_v4().simple()),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let location_id = seed_location(&db, admin_id).await;

    let rule_id = Uuid::new_v4();
    let past = Utc::now() - chrono::Duration::seconds(5);
    sqlx::query(
        "INSERT INTO recurring_session_rules \
         (id, location_id, batch_id, description, duration_minutes, run_times_local, timezone, \
          days_of_week, is_active, created_by, created_at, updated_at, locked_short_code, next_run_at, \
          monitoring_enabled, class_duration_minutes) \
         VALUES ($1, $2, NULL, 'monitoring propagation test rule', 30, ARRAY['09:00'::time], 'UTC', \
                 ARRAY[0,1,2,3,4,5,6]::smallint[], true, $3, now(), now(), NULL, $4, true, 120)",
    )
    .bind(rule_id)
    .bind(location_id)
    .bind(admin_id)
    .bind(past)
    .execute(&db)
    .await
    .expect("insert due, monitoring-enabled recurring rule");

    let claimed = attendance_geotag_backend::services::claim_and_generate_one(&db)
        .await
        .expect("claim pass should not error");
    assert!(claimed, "the due rule should have been claimed");

    let (monitoring_enabled, class_duration_minutes, monitoring_ends_at): (
        bool,
        Option<i32>,
        Option<chrono::DateTime<Utc>>,
    ) = sqlx::query_as(
        "SELECT monitoring_enabled, class_duration_minutes, monitoring_ends_at \
         FROM sessions WHERE recurring_rule_id = $1",
    )
    .bind(rule_id)
    .fetch_one(&db)
    .await
    .expect("fetch generated session's monitoring fields");

    assert!(
        monitoring_enabled,
        "generated session should inherit monitoring_enabled"
    );
    assert_eq!(class_duration_minutes, Some(120));
    assert!(
        monitoring_ends_at.is_some(),
        "generated session should have monitoring_ends_at computed"
    );
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn update_session_schedule_same_day_succeeds_and_updates_monitoring() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let location_id = seed_location(&db, admin_id).await;

    let (status, created) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {created:?}");
    let session_id = created["_id"].as_str().unwrap().to_string();

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{session_id}/schedule"),
            serde_json::json!({
                "monitoringEnabled": true,
                "classDurationMinutes": 75,
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "response body: {body:?}");
    assert_eq!(body["monitoringEnabled"], serde_json::json!(true));
    assert_eq!(body["classDurationMinutes"], serde_json::json!(75));
    assert!(!body["monitoringEndsAt"].is_null());
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn update_session_schedule_rejects_edit_on_a_later_day() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = login_super_admin(&app, &db).await;
    let location_id = seed_location(&db, admin_id).await;

    let (status, created) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {created:?}");
    let session_id = created["_id"].as_str().unwrap().to_string();

    // Backdate the session's anchor (created_at, since this session has no
    // starts_at) to yesterday, simulating an edit attempted the day after
    // the class actually happened.
    sqlx::query("UPDATE sessions SET created_at = now() - interval '1 day' WHERE id = $1")
        .bind(Uuid::parse_str(&session_id).unwrap())
        .execute(&db)
        .await
        .unwrap();

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{session_id}/schedule"),
            serde_json::json!({
                "monitoringEnabled": true,
                "classDurationMinutes": 60,
            }),
        )
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "editing a session's schedule on a later day must be rejected: {body:?}"
    );
}
