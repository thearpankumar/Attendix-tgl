//! Short-link code hygiene / re-attach expiry, and read-only exports for
//! assigned mentors.

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

const PW: &str = "correct-horse-battery-staple";

async fn client_for(app: &axum::Router, db: &sqlx::PgPool, role: &str) -> (Client, Uuid) {
    let u = unique(if role == "admin" {
        "slx-mentor"
    } else {
        "slx-super"
    });
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, role).await;
    (Client::login(app, &u, PW).await, id)
}

async fn create_session(client: &Client, app: &axum::Router, location: Uuid) -> serde_json::Value {
    let (status, s) = client
        .mutate(
            app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({ "locationId": location.to_string(), "durationMinutes": 30, "shortlinkMode": "none" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{s:?}");
    s
}

/// F11: custom codes are normalised and validated on create (so every other
/// endpoint, which lower-cases the path, can find them), and a malformed
/// session id is a 400 instead of silently meaning "no session".
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn short_link_create_normalises_and_validates() {
    let (app, db) = create_test_app().await;
    let (client, _) = client_for(&app, &db, "super_admin").await;
    let suffix = &Uuid::new_v4().simple().to_string()[..6];

    let mixed = format!("MixedCase-{suffix}");
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/shortlinks",
            serde_json::json!({ "shortCode": mixed }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let stored = format!("mixedcase-{suffix}");
    assert_eq!(body["shortCode"], stored, "{body:?}");
    // And it is manageable by the code the admin typed.
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/shortlinks/{mixed}/detach"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "stored code must be manageable: {body:?}"
    );
    // Case-insensitive duplicate.
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/shortlinks",
            serde_json::json!({ "shortCode": stored.to_uppercase() }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    for bad in [
        "ab",
        "has space",
        "under_score",
        "dot.dot",
        "ünïcode",
        &"x".repeat(21),
    ] {
        let (status, body) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/shortlinks",
                serde_json::json!({ "shortCode": bad }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}: {body:?}");
    }

    // Blank code => generated; blank/absent session id => unattached.
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/shortlinks",
            serde_json::json!({ "shortCode": "", "sessionId": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert!(body["sessionId"].is_null());

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/shortlinks",
            serde_json::json!({ "sessionId": "not-a-uuid" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let (status, body) = client
        .get(&app, "/api/admin/shortlinks?session_id=not-a-uuid")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
}

/// F12: re-attaching a link whose previous session expired gives it the new
/// session's expiry, so the public resolver stops 404-ing it.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn reattaching_an_expired_link_adopts_the_new_sessions_expiry() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = client_for(&app, &db, "super_admin").await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "SL001").await;
    let code = format!("reuse-{}", &Uuid::new_v4().simple().to_string()[..6]);

    let old = create_session(&client, &app, location_id).await;
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/shortlinks",
            serde_json::json!({ "shortCode": code, "sessionId": old["_id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    // The old session ended and its link expired with it.
    sqlx::query("UPDATE sessions SET is_active = false WHERE id = $1")
        .bind(Uuid::parse_str(old["_id"].as_str().unwrap()).unwrap())
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE short_links SET expires_at = now() - interval '1 day' WHERE short_code = $1",
    )
    .bind(&code)
    .execute(&db)
    .await
    .unwrap();

    let new = create_session(&client, &app, location_id).await;
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/shortlinks/{code}/attach"),
            serde_json::json!({ "sessionId": new["_id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (link_exp, session_exp): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as(
            "SELECT l.expires_at, s.expires_at FROM short_links l \
             JOIN sessions s ON s.id = l.session_id WHERE l.short_code = $1",
        )
        .bind(&code)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(link_exp, session_exp);
    assert!(link_exp > chrono::Utc::now());

    // Student-facing resolver no longer treats it as expired.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/s/{code}"))
                .header("user-agent", "Mozilla/5.0 (Linux; Android 13; Mobile)")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

/// F17: a mentor assigned to an exam session may export it (single and bulk,
/// read-only); an unrelated mentor still can't.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn assigned_mentor_can_export_but_unrelated_mentor_cannot() {
    let (app, db) = create_test_app().await;
    let (sup, sup_id) = client_for(&app, &db, "super_admin").await;
    let (assigned, assigned_id) = client_for(&app, &db, "admin").await;
    let (other, _) = client_for(&app, &db, "admin").await;
    let (location_id, batch_id) = seed_location_and_batch(&db, sup_id, "EX001").await;

    let (status, session) = sup
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [assigned_id],
                "collegeName": "XYZ College",
                "startsAt": (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339(),
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session:?}");
    let sid = session["_id"].as_str().unwrap().to_string();

    // Assigned mentor: allowed to read the roster (baseline) and now export.
    let (status, _) = assigned
        .get(&app, &format!("/api/admin/sessions/{sid}/roster"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = assigned
        .get(&app, &format!("/api/admin/sessions/{sid}/export"))
        .await;
    assert_eq!(status, StatusCode::OK, "assigned mentor single export");
    let (status, _) = assigned
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions/export-bulk",
            serde_json::json!({ "sessionIds": [sid] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "assigned mentor bulk export");

    // Unrelated mentor: 404 on single, nothing exportable in bulk.
    let (status, _) = other
        .get(&app, &format!("/api/admin/sessions/{sid}/export"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions/export-bulk",
            serde_json::json!({ "sessionIds": [sid] }),
        )
        .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "unrelated mentor must not get the workbook"
    );

    // Assignment is read-only: still no schedule/mentor edits.
    let (status, _) = assigned
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{sid}/schedule"),
            serde_json::json!({ "durationMinutes": 30 }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
