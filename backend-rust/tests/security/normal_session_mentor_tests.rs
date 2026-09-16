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

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serial_test::file_serial;
use tower::ServiceExt;
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

/// Reported bug: a mentor re-marking a student on a normal session (absent
/// then present, or present then absent) looked correct in the mentor app
/// but the super-admin's session-detail page (`/stats` and `/absent`) stayed
/// frozen at whatever the *first* mark had been. Root cause: those two
/// endpoints treated "an attendance row exists for this roll number" as
/// "present", never looking at the row's `status` — and `mark_attendance_manual`
/// updates the same row in place rather than deleting/recreating it, so once
/// any row existed the student never went back to "absent" no matter how
/// `status` changed afterwards.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn normal_session_stats_track_mentor_remarking_not_just_first_mark() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-remark");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let (mentor_id, mentor_username) = create_mentor(&super_client, &app, "nrm-mentor-remark").await;

    let roll_number = "NORM-REMARK-1";
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

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;

    // First mark: absent. Admin's stats/absent list must show it immediately.
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "absent" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stats) = super_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/stats"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        stats["absentCount"], 1,
        "super-admin stats must show the student absent right after the first mark: {stats:?}"
    );

    // Re-mark: present. This is the flip the bug report describes — admin
    // side must update, not stay stuck on the first mark.
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "present" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stats) = super_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/stats"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        stats["absentCount"], 0,
        "super-admin stats must reflect the re-mark to present: {stats:?}"
    );
    let (status, absent) = super_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/absent"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        absent.as_array().unwrap().len(),
        0,
        "absent list must no longer include a student re-marked present: {absent:?}"
    );

    // And the reverse flip, to confirm it isn't one-way.
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "absent" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stats) = super_client
        .get(&app, &format!("/api/admin/sessions/{session_id}/stats"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        stats["absentCount"], 1,
        "super-admin stats must reflect the re-mark back to absent: {stats:?}"
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

/// Guards against a regression the fix above could plausibly have caused:
/// making `get_session_stats`/`get_session_absent` look at `status` instead
/// of "does a row exist" must not touch the *student-facing* "already
/// submitted" gate at all — `check_attendance_status` and `submit_attendance`
/// both still key off row existence alone (see attendance.rs), so a student
/// must stay locked out of self-submitting a second time regardless of
/// whether a mentor's manual mark on that row currently reads present or
/// absent.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn student_stays_locked_out_after_mentor_marks_present_or_absent() {
    let (app, db) = create_test_app().await;

    let super_username = unique("nrm-super-lock");
    let super_id = seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let (mentor_id, mentor_username) = create_mentor(&super_client, &app, "nrm-mentor-lock").await;

    let roll_number = "NORM-LOCK-1";
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
    let token = session_body["token"].as_str().unwrap().to_string();

    let check_status = || {
        let app = app.clone();
        let token = token.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .uri(format!(
                            "/api/attend/{token}/status?rollNumber={roll_number}"
                        ))
                        .header("user-agent", "Mozilla/5.0 (Linux; Android 13; Mobile)")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            let body: serde_json::Value =
                serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
            (status, body)
        }
    };

    // Before any mark: the student is free to submit.
    let (status, body) = check_status().await;
    assert_eq!(status, StatusCode::OK, "status check should load: {body:?}");
    assert_eq!(
        body["alreadySubmitted"], false,
        "no mark yet — student should not be locked out: {body:?}"
    );

    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "absent" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = check_status().await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["alreadySubmitted"], true,
        "mentor marked absent — student must be locked out, not free to self-submit: {body:?}"
    );

    // Flip to present — still locked out; only the mentor may change it.
    let (status, _) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll_number, "status": "present" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = check_status().await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["alreadySubmitted"], true,
        "mentor marked present — student must still be locked out: {body:?}"
    );

    // Belt and suspenders: the actual submission endpoint never returns
    // success here either (captcha is required first in production config,
    // so this mainly confirms there's no accidental early-return success
    // path before that check is even reached).
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/attend/{token}"))
                .header("content-type", "application/json")
                .header("user-agent", "Mozilla/5.0 (Linux; Android 13; Mobile)")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "rollNumber": roll_number,
                        "studentName": "Test Student",
                        "photoUrl": "https://example.com/p.jpg",
                        "photoPublicId": "photos/p",
                        "latitude": 12.9716,
                        "longitude": 77.5946,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        response.status(),
        StatusCode::OK,
        "a student must never be able to self-submit over an existing (mentor-marked) row"
    );
}
