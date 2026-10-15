//! One-time scheduled sessions (`runOnceDate`) and cron `startDate` over the
//! real `/api/admin/recurring-rules` API, plus the scheduler deactivating a
//! one-time rule after it fires.

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn setup() -> (axum::Router, sqlx::PgPool, Client, Uuid, Uuid) {
    let (app, db) = create_test_app().await;
    let username = format!("onetime-admin-{}", Uuid::new_v4().simple());
    let email = format!("{username}@example.com");
    seed_admin(
        &db,
        &username,
        &email,
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
        .bind(&username)
        .fetch_one(&db)
        .await
        .unwrap();
    let location_id: Uuid = sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, 'One Time Test Location', 12.9716, 77.5946, 100.0, $2) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    (app, db, client, admin_id, location_id)
}

fn payload(location_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "locationId": location_id.to_string(),
        "description": "one-time test",
        "durationMinutes": 30,
        "runTimesLocal": ["09:00"],
        "timezone": "UTC",
        "shortlinkMode": "none",
    })
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn one_time_rule_is_created_for_a_future_date_without_days_of_week() {
    let (app, db, client, _admin, location_id) = setup().await;
    let date = (Utc::now() + Duration::days(3)).date_naive();
    let mut p = payload(location_id);
    p["runOnceDate"] = serde_json::json!(date.to_string());
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["runOnceDate"], serde_json::json!(date.to_string()));
    assert_eq!(body["completed"], serde_json::json!(false));
    let next = body["nextRunAt"].as_str().unwrap();
    assert!(next.starts_with(&format!("{date}T09:00:00")), "{next}");
    let _ = db;
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn one_time_rule_rejects_past_dates_and_multiple_times_and_start_date_mix() {
    let (app, _db, client, _admin, location_id) = setup().await;

    let mut p = payload(location_id);
    p["runOnceDate"] = serde_json::json!((Utc::now() - Duration::days(2)).date_naive().to_string());
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");

    let future = (Utc::now() + Duration::days(2)).date_naive().to_string();
    let mut p = payload(location_id);
    p["runOnceDate"] = serde_json::json!(future);
    p["runTimesLocal"] = serde_json::json!(["09:00", "10:00"]);
    let (status, _) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut p = payload(location_id);
    p["runOnceDate"] = serde_json::json!(future);
    p["startDate"] = serde_json::json!(future);
    let (status, _) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A one-time rule earlier today (UTC) is in the past.
    let mut p = payload(location_id);
    p["runTimesLocal"] = serde_json::json!(["00:00"]);
    p["runOnceDate"] = serde_json::json!(Utc::now().date_naive().to_string());
    let (status, _) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn cron_rule_accepts_today_as_start_date_and_rejects_past() {
    let (app, _db, client, _admin, location_id) = setup().await;
    let today = Utc::now().date_naive();

    let mut p = payload(location_id);
    p["daysOfWeek"] = serde_json::json!([0, 1, 2, 3, 4, 5, 6]);
    p["startDate"] = serde_json::json!(today.to_string());
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["startDate"], serde_json::json!(today.to_string()));

    let mut p = payload(location_id);
    p["daysOfWeek"] = serde_json::json!([0, 1, 2, 3, 4, 5, 6]);
    p["startDate"] = serde_json::json!((today - Duration::days(1)).to_string());
    let (status, _) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A recurring rule still needs days of week.
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/recurring-rules",
            payload(location_id),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn scheduler_fires_a_due_one_time_rule_once_then_deactivates_it() {
    let (app, db, client, admin_id, location_id) = setup().await;
    let _ = (&app, &client);
    let rule_id = Uuid::new_v4();
    let today = Utc::now().date_naive();
    sqlx::query(
        "INSERT INTO recurring_session_rules \
         (id, location_id, description, duration_minutes, run_times_local, timezone, days_of_week, \
          is_active, created_by, next_run_at, run_once_date) \
         VALUES ($1, $2, 'one-time fire', 30, ARRAY['09:00'::time], 'UTC', ARRAY[0]::smallint[], \
                 true, $3, now() - interval '5 seconds', $4)",
    )
    .bind(rule_id)
    .bind(location_id)
    .bind(admin_id)
    .bind(today)
    .execute(&db)
    .await
    .unwrap();

    // Drain: other due rules from this file's tests may exist; loop until ours fired.
    while attendance_geotag_backend::services::claim_and_generate_one(&db)
        .await
        .unwrap()
    {}

    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE recurring_rule_id = $1")
            .bind(rule_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(sessions, 1);
    let (active, generated): (bool, Option<Uuid>) = sqlx::query_as(
        "SELECT is_active, last_generated_session_id FROM recurring_session_rules WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!active, "one-time rule must deactivate after firing");
    assert!(generated.is_some());

    // Second drain must not generate again.
    while attendance_geotag_backend::services::claim_and_generate_one(&db)
        .await
        .unwrap()
    {}
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE recurring_rule_id = $1")
            .bind(rule_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(sessions, 1);

    // A completed one-time rule cannot be resumed.
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/resume"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
