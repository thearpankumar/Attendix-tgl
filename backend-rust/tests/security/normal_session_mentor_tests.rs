//! End-to-end behavioural tests for mentors assigned to a *normal*
//! (student self-service, geofenced) session — as opposed to the
//! exam-session mentor flow already covered by `exam_session_flow_tests.rs`,
//! which these tests deliberately never touch or modify.
//!
//! Covers: assigning a mentor to a normal session at creation and
//! afterwards; a mentor marking/undoing attendance on a normal session even
//! after that session's own (short) self-check-in window has closed, as long
//! as it's within the configurable mentor-edit window counted from
//! `created_at`; and that window closing for good — locking both the mentor
//! list and attendance edits — once it passes.

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

/// Backdates a session's `created_at` by the given number of hours — the
/// same technique `session_monitoring_tests.rs` uses to simulate an edit
/// attempted long after a session was created, since the API itself never
/// lets a caller set `created_at` directly.
async fn backdate_session_created_at(db: &sqlx::PgPool, session_id: &str, hours_ago: i64) {
    sqlx::query("UPDATE sessions SET created_at = now() - ($2 || ' hours')::interval WHERE id = $1")
        .bind(Uuid::parse_str(session_id).unwrap())
        .bind(hours_ago.to_string())
        .execute(db)
        .await
        .unwrap();
}

async fn create_mentor(super_client: &Client, app: &axum::Router, prefix: &str) -> (String, String) {
    let username = unique(prefix);
    let (status, body) = super_client
        .mutate(
            app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": username,
                "email": format!("{}@example.com", username),
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "creating mentor {username} should succeed: {body:?}"
    );
    let mentor_id = body["_id"].as_str().unwrap().to_string();
    (mentor_id, username)
}

/// A normal session may optionally carry a mentor at creation time, without
/// being reclassified as an exam session (still geofenced, still no
/// college/starts_at requirement). The assigned mentor can see it; an
/// unassigned mentor cannot.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn normal_session_accepts_optional_mentor_at_creation() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-a");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let (mentor_id, mentor_username) = create_mentor(&super_client, &app, "nrm-mentor-a").await;

    let roll_number = "NORM001";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "sessionType": "normal",
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "normal session with an optional mentor should succeed: {session_body:?}"
    );
    assert_eq!(
        session_body["locationId"].as_str().unwrap(),
        location_id.to_string(),
        "a normal session must keep its location, unlike an exam session"
    );
    assert_eq!(
        session_body["assignedAdminIds"].as_array().unwrap().len(),
        1
    );

    let session_id = session_body["_id"].as_str().unwrap().to_string();

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;
    let (status, _) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the assigned mentor should see the normal session's roster"
    );

    let (outsider_id, outsider_username) =
        create_mentor(&super_client, &app, "nrm-outside-a").await;
    let _ = outsider_id;
    let outsider_client = Client::login(&app, &outsider_username, "mentor-password-123").await;
    let (status, _) = outsider_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unassigned mentor must not see this normal session"
    );
}

/// A normal session created with no mentors at all is still valid (mentors
/// are optional, not required, unlike an exam session) and can have one
/// added afterwards via the mentor add/remove endpoint.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn normal_session_mentor_can_be_assigned_after_creation() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-b");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let roll_number = "NORM002";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "sessionType": "normal",
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {session_body:?}");
    assert!(
        session_body["assignedAdminIds"].as_array().unwrap().is_empty(),
        "a normal session should be creatable with zero mentors"
    );
    let session_id = session_body["_id"].as_str().unwrap().to_string();

    let (mentor_id, mentor_username) =
        create_mentor(&super_client, &app, "nrm-mentor-b").await;

    let (status, patch_body) = super_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{session_id}/mentors"),
            serde_json::json!({ "add": [mentor_id] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "adding a mentor to a fresh normal session should succeed: {patch_body:?}"
    );
    assert_eq!(patch_body["assignedAdminIds"].as_array().unwrap().len(), 1);

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;
    let (status, _) = mentor_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(status, StatusCode::OK, "newly assigned mentor should see the roster");
}

/// The core of the feature: a normal session's own self-check-in window
/// (`expiresAt`) can close quickly, but a mentor must still be able to fix a
/// student's attendance well after that, up to the separate, later
/// mentor-edit-window cutoff counted from `created_at`.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn mentor_can_mark_normal_session_after_its_own_window_closed_but_within_48h() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-c");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let (mentor_id, mentor_username) = create_mentor(&super_client, &app, "nrm-mentor-c").await;

    let roll_number = "NORM003";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "sessionType": "normal",
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "durationMinutes": 5,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {session_body:?}");
    let session_id = session_body["_id"].as_str().unwrap().to_string();

    // Simulate the session's own 5-minute self-check-in window having
    // already closed, without touching `created_at` — the mentor-edit
    // window is anchored to creation, not to this.
    sqlx::query("UPDATE sessions SET expires_at = now() - interval '1 hour' WHERE id = $1")
        .bind(Uuid::parse_str(&session_id).unwrap())
        .execute(&db)
        .await
        .unwrap();

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
        StatusCode::OK,
        "mentor should still be able to mark a normal session whose own window closed, \
         as long as it's within the mentor-edit window: {mark_body:?}"
    );

    let (status, _) = mentor_client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/sessions/{session_id}/attendance/manual/{roll_number}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "undo should likewise be allowed after the session's own window closed"
    );
}

/// Once the configurable mentor-edit window has passed since the session was
/// *created*, marking, undoing, and add/remove-mentor are all rejected —
/// even though nothing here ever depends on the session's own (already long
/// closed) self-check-in window.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn normal_session_locks_mentor_edits_after_48h_window() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-d");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let (mentor_id, mentor_username) = create_mentor(&super_client, &app, "nrm-mentor-d").await;

    let roll_number = "NORM004";
    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, roll_number).await;

    let (status, session_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "sessionType": "normal",
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "response body: {session_body:?}");
    let session_id = session_body["_id"].as_str().unwrap().to_string();

    // Push the session's creation time back beyond the default 48h window
    // (SystemConfig::default().session_config.mentor_edit_window_hours).
    backdate_session_created_at(&db, &session_id, 49).await;

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
        "marking must be rejected once the mentor-edit window has closed: {mark_body:?}"
    );

    let (status, undo_body) = mentor_client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/sessions/{session_id}/attendance/manual/{roll_number}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "undo must likewise be rejected once the mentor-edit window has closed: {undo_body:?}"
    );

    let (another_mentor_id, _) =
        create_mentor(&super_client, &app, "nrm-mentor-e").await;
    let (status, add_body) = super_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{session_id}/mentors"),
            serde_json::json!({ "add": [another_mentor_id] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "adding a mentor must be rejected once the window has closed: {add_body:?}"
    );

    let (status, remove_body) = super_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{session_id}/mentors"),
            serde_json::json!({ "remove": [mentor_id] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "removing an already-assigned mentor must also be rejected once the window has closed: {remove_body:?}"
    );
}
