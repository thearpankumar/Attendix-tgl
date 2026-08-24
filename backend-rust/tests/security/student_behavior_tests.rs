//! Regression coverage for the Phase 4 "view student behavior" summary
//! endpoint: `GET /api/admin/sessions/{id}/students/{rollNumber}/behavior`.
//! No real extension traffic exists in tests, so device locks and telemetry
//! events are inserted directly — the same "synthetic event generator"
//! technique already used in telemetry_ingestion_tests.rs.

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Student Behavior Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_lock(db: &sqlx::PgPool, session_id: Uuid, roll_number: &str, status: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO session_device_locks \
         (id, session_id, roll_number, extension_instance_id, locked_at, status) \
         VALUES ($1, $2, $3, $4, now(), $5)",
    )
    .bind(id)
    .bind(session_id)
    .bind(roll_number)
    .bind(Uuid::new_v4())
    .bind(status)
    .execute(db)
    .await
    .unwrap();
    id
}

async fn insert_event(
    timescale_db: &sqlx::PgPool,
    session_id: Uuid,
    roll_number: &str,
    event_type: &str,
    event_data: serde_json::Value,
    recorded_at: chrono::DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO telemetry_events \
         (id, session_id, roll_number, extension_instance_id, event_type, event_data, recorded_at, received_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll_number)
    .bind(Uuid::new_v4())
    .bind(event_type)
    .bind(event_data)
    .bind(recorded_at)
    .execute(timescale_db)
    .await
    .unwrap();
}

async fn create_session(
    app: &axum::Router,
    client: &Client,
    db: &sqlx::PgPool,
    admin_id: Uuid,
) -> Uuid {
    let location_id = seed_location(db, admin_id).await;
    let (status, body) = client
        .mutate(
            app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({ "locationId": location_id.to_string(), "durationMinutes": 30 }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
    Uuid::parse_str(body["_id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn summary_includes_device_locks_and_computed_behavior_summary() {
    let (app, db) = create_test_app().await;
    let timescale_db = crate::test_db::get_test_timescale_database().await;
    let username = format!("behavior-admin-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let session_id = create_session(&app, &client, &db, admin_id).await;
    let roll_number = "ROLL010";

    let old_lock = insert_lock(&db, session_id, roll_number, "superseded").await;
    let active_lock = insert_lock(&db, session_id, roll_number, "active").await;

    let t0 = Utc::now() - Duration::minutes(10);
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "idle_state",
        serde_json::json!({ "state": "idle" }),
        t0,
    )
    .await;
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "idle_state",
        serde_json::json!({ "state": "active" }),
        t0 + Duration::seconds(90),
    )
    .await;
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "window_focus",
        serde_json::json!({ "focused": false }),
        t0 + Duration::seconds(100),
    )
    .await;
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "window_focus",
        serde_json::json!({ "focused": true }),
        t0 + Duration::seconds(110),
    )
    .await;
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "tab_url_change",
        serde_json::json!({ "domain": "meet.google.com" }),
        t0 + Duration::seconds(120),
    )
    .await;
    insert_event(
        &timescale_db,
        session_id,
        roll_number,
        "meet_tab_open",
        serde_json::json!({ "open": true }),
        t0 + Duration::seconds(121),
    )
    .await;
    // A different student's event must never leak into this roll number's summary.
    insert_event(
        &timescale_db,
        session_id,
        "ROLL999",
        "heartbeat",
        serde_json::json!({}),
        t0,
    )
    .await;

    let (status, body) = client
        .get(
            &app,
            &format!("/api/admin/sessions/{session_id}/students/{roll_number}/behavior"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");

    assert_eq!(body["rollNumber"], serde_json::json!(roll_number));

    let locks = body["deviceLocks"].as_array().unwrap();
    assert_eq!(
        locks.len(),
        2,
        "both the superseded and active lock must be visible: {body:?}"
    );
    let lock_ids: Vec<String> = locks
        .iter()
        .map(|l| l["id"].as_str().unwrap().to_string())
        .collect();
    assert!(lock_ids.contains(&old_lock.to_string()));
    assert!(lock_ids.contains(&active_lock.to_string()));

    let events = body["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        6,
        "only this roll number's events, chronological: {body:?}"
    );
    assert_eq!(events[0]["eventType"], serde_json::json!("idle_state"));
    assert_eq!(events[5]["eventType"], serde_json::json!("meet_tab_open"));

    let summary = &body["summary"];
    assert_eq!(summary["totalIdleSeconds"], serde_json::json!(90));
    assert_eq!(summary["focusTransitionCount"], serde_json::json!(2));
    assert_eq!(summary["tabSwitchCount"], serde_json::json!(1));
    assert_eq!(summary["meetTabEverOpen"], serde_json::json!(true));
    assert_eq!(summary["eventCount"], serde_json::json!(6));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn super_admin_can_view_behavior_for_any_session() {
    let (app, db) = create_test_app().await;
    let username = format!("behavior-super-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let session_id = create_session(&app, &client, &db, admin_id).await;

    // A second, unrelated super-admin — super-admins are peers with full
    // visibility, same as every other session sub-resource.
    let other_username = format!("behavior-super-2-{}", Uuid::new_v4().simple());
    seed_admin(
        &db,
        &other_username,
        &format!("{other_username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let other_client = Client::login(&app, &other_username, "correct-horse-battery-staple").await;

    let (status, body) = other_client
        .get(
            &app,
            &format!("/api/admin/sessions/{session_id}/students/ROLL011/behavior"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn mentor_not_assigned_to_the_session_cannot_view_its_behavior() {
    let (app, db) = create_test_app().await;
    let username = format!("behavior-owner-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let session_id = create_session(&app, &client, &db, admin_id).await;

    let mentor_username = format!("behavior-mentor-{}", Uuid::new_v4().simple());
    seed_admin(
        &db,
        &mentor_username,
        &format!("{mentor_username}@example.com"),
        "correct-horse-battery-staple",
        "admin",
    )
    .await;
    let mentor_client = Client::login(&app, &mentor_username, "correct-horse-battery-staple").await;

    // This mentor was never assigned to `session_id` via session_admins.
    let (status, body) = mentor_client
        .get(
            &app,
            &format!("/api/admin/sessions/{session_id}/students/ROLL012/behavior"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "response body: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn mentor_assigned_to_the_session_can_view_its_behavior() {
    let (app, db) = create_test_app().await;
    let username = format!("behavior-owner-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let session_id = create_session(&app, &client, &db, admin_id).await;

    let mentor_username = format!("behavior-mentor-{}", Uuid::new_v4().simple());
    let mentor_id = seed_admin(
        &db,
        &mentor_username,
        &format!("{mentor_username}@example.com"),
        "correct-horse-battery-staple",
        "admin",
    )
    .await;
    let mentor_client = Client::login(&app, &mentor_username, "correct-horse-battery-staple").await;

    sqlx::query("INSERT INTO session_admins (session_id, admin_id) VALUES ($1, $2)")
        .bind(session_id)
        .bind(mentor_id)
        .execute(&db)
        .await
        .unwrap();

    let (status, body) = mentor_client
        .get(
            &app,
            &format!("/api/admin/sessions/{session_id}/students/ROLL013/behavior"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "response body: {body:?}");
}
