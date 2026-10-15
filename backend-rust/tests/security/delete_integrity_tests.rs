//! Deleting things that other rows still point at must either work cleanly
//! (sessions) or be refused with a clear 409 (locations, rule-locked short
//! links) — never an opaque 500, and never after already destroying data.
//! Also covers pause/resume keeping a recurring rule's short link.

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn super_client(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let u = unique("delint-super");
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    (Client::login(app, &u, PW).await, id)
}

async fn count(db: &sqlx::PgPool, sql: &str, id: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn insert_attendance(db: &sqlx::PgPool, session_id: Uuid, roll: &str) {
    sqlx::query(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, \
            source, status, captured_at \
         ) VALUES ($1, $2, 'Test Student', $3, 'https://example.com/p.jpg', '', 12.97, 77.59, 0.0, true, \
            'self_submitted', 'present', now())",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll)
    .execute(db)
    .await
    .unwrap();
}

async fn create_session(client: &Client, app: &axum::Router, location_id: Uuid) -> Uuid {
    let (status, session) = client
        .mutate(
            app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session:?}");
    Uuid::parse_str(session["_id"].as_str().unwrap()).unwrap()
}

async fn create_rule(
    client: &Client,
    app: &axum::Router,
    location_id: Uuid,
    extra: serde_json::Value,
) -> Uuid {
    let mut payload = serde_json::json!({
        "locationId": location_id.to_string(),
        "durationMinutes": 30,
        "runTimesLocal": ["09:00"],
        "daysOfWeek": [0, 1, 2, 3, 4, 5, 6],
        "timezone": "UTC",
        "shortlinkMode": "none",
    });
    for (k, v) in extra.as_object().unwrap() {
        payload[k] = v.clone();
    }
    let (status, rule) = client
        .mutate(app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rule:?}");
    Uuid::parse_str(rule["_id"].as_str().unwrap()).unwrap()
}

fn code() -> String {
    format!("di-{}", &Uuid::new_v4().simple().to_string()[..8])
}

/// F1: the rule's most recent occurrence is the session that
/// `recurring_session_rules.last_generated_session_id` points at. Deleting it
/// used to 500 on that FK *after* the attendance rows were already gone.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn deleting_a_rules_latest_generated_session_succeeds() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI001").await;
    let rule_id = create_rule(&client, &app, location_id, serde_json::json!({})).await;

    for bulk in [false, true] {
        let session_id = create_session(&client, &app, location_id).await;
        insert_attendance(&db, session_id, "DI001").await;
        sqlx::query("UPDATE sessions SET recurring_rule_id = $1 WHERE id = $2")
            .bind(rule_id)
            .bind(session_id)
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE recurring_session_rules SET last_generated_session_id = $1 WHERE id = $2",
        )
        .bind(session_id)
        .bind(rule_id)
        .execute(&db)
        .await
        .unwrap();

        let (status, body) = if bulk {
            client
                .mutate(
                    &app,
                    "POST",
                    "/api/admin/sessions/delete-bulk",
                    serde_json::json!({ "sessionIds": [session_id.to_string()], "password": PW }),
                )
                .await
        } else {
            client
                .mutate(
                    &app,
                    "DELETE",
                    &format!("/api/admin/sessions/{session_id}"),
                    serde_json::json!({ "password": PW }),
                )
                .await
        };
        assert_eq!(status, StatusCode::OK, "bulk={bulk}: {body:?}");
        if bulk {
            assert_eq!(body["deletedCount"], 1, "{body:?}");
            assert_eq!(body["failedIds"].as_array().unwrap().len(), 0, "{body:?}");
        }
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM sessions WHERE id = $1",
                session_id
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM attendances WHERE session_id = $1",
                session_id
            )
            .await,
            0
        );
        let pointer: Option<Uuid> = sqlx::query_scalar(
            "SELECT last_generated_session_id FROM recurring_session_rules WHERE id = $1",
        )
        .bind(rule_id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(
            pointer.is_none(),
            "pointer must be cleared, not left dangling"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM recurring_session_rules WHERE id = $1",
                rule_id
            )
            .await,
            1,
            "the rule itself survives"
        );
    }
}

/// F1: the delete is one transaction. A reference this code doesn't know about
/// must roll everything back (attendance, short-link detach) rather than leave
/// a half-deleted session behind a 500.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn session_delete_is_atomic_when_something_still_references_it() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI002").await;
    let session_id = create_session(&client, &app, location_id).await;
    insert_attendance(&db, session_id, "DI002").await;
    let link = code();
    sqlx::query(
        "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, click_count, created_at) \
         VALUES ($1, $2, $3, $4, true, 0, now())",
    )
    .bind(Uuid::new_v4())
    .bind(&link)
    .bind(session_id)
    .bind(admin_id)
    .execute(&db)
    .await
    .unwrap();

    // An unknown FK to sessions, standing in for any future table.
    sqlx::query("DROP TABLE IF EXISTS zz_delint_fk_probe")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE zz_delint_fk_probe (session_id UUID REFERENCES sessions(id))")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO zz_delint_fk_probe VALUES ($1)")
        .bind(session_id)
        .execute(&db)
        .await
        .unwrap();

    let (status, _) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/sessions/{session_id}"),
            serde_json::json!({ "password": PW }),
        )
        .await;
    let attendance_left = count(
        &db,
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1",
        session_id,
    )
    .await;
    let link_session: Option<Uuid> =
        sqlx::query_scalar("SELECT session_id FROM short_links WHERE short_code = $1")
            .bind(&link)
            .fetch_one(&db)
            .await
            .unwrap();
    sqlx::query("DROP TABLE zz_delint_fk_probe")
        .execute(&db)
        .await
        .unwrap();

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        attendance_left, 1,
        "attendance must survive a failed delete"
    );
    assert_eq!(
        link_session,
        Some(session_id),
        "short link must stay attached"
    );
}

/// F2: a location that sessions/rules reference can't be deleted (409 with an
/// impact message, nothing changed); an unreferenced one still can.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn deleting_an_in_use_location_is_a_409_and_unused_one_deletes() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (used_by_session, _) = seed_location_and_batch(&db, admin_id, "DI003").await;
    let (used_by_rule, _) = seed_location_and_batch(&db, admin_id, "DI004").await;
    let (unused, _) = seed_location_and_batch(&db, admin_id, "DI005").await;
    create_session(&client, &app, used_by_session).await;
    create_rule(&client, &app, used_by_rule, serde_json::json!({})).await;

    for (loc, expect) in [
        (used_by_session, "1 session(s) and 0 recurring rule(s)"),
        (used_by_rule, "0 session(s) and 1 recurring rule(s)"),
    ] {
        let (status, body) = client
            .mutate(
                &app,
                "DELETE",
                &format!("/api/admin/locations/{loc}"),
                serde_json::json!({}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
        let msg = body["message"].as_str().unwrap();
        assert!(msg.contains(expect), "{msg}");
        assert!(msg.to_lowercase().contains("deactivate"), "{msg}");
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM locations WHERE id = $1", loc).await,
            1
        );
    }

    // Deactivating (the suggested alternative) works and keeps history.
    let (status, body) = client
        .mutate(
            &app,
            "PUT",
            &format!("/api/admin/locations/{used_by_session}"),
            serde_json::json!({ "isActive": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["isActive"], false);

    let (status, body) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/locations/{unused}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM locations WHERE id = $1", unused).await,
        0
    );
}

/// F2/F16: new sessions/rules (and re-pointing a rule) need a location that
/// exists and is active — 400, not a 500 from the FK.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn unknown_or_deactivated_locations_are_rejected_on_create_and_patch() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI006").await;
    let (inactive_id, _) = seed_location_and_batch(&db, admin_id, "DI007").await;
    sqlx::query("UPDATE locations SET is_active = false WHERE id = $1")
        .bind(inactive_id)
        .execute(&db)
        .await
        .unwrap();
    let rule_id = create_rule(&client, &app, location_id, serde_json::json!({})).await;
    let bogus = Uuid::new_v4();

    for (loc, needle) in [(bogus, "not found"), (inactive_id, "deactivated")] {
        let (status, body) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/sessions",
                serde_json::json!({ "locationId": loc.to_string(), "durationMinutes": 30 }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "session {loc}: {body:?}");
        assert!(
            body["message"].as_str().unwrap().contains(needle),
            "{body:?}"
        );

        let (status, body) = client
            .mutate(
                &app,
                "PATCH",
                &format!("/api/admin/recurring-rules/{rule_id}"),
                serde_json::json!({ "locationId": loc.to_string() }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "patch {loc}: {body:?}");
        assert!(
            body["message"].as_str().unwrap().contains(needle),
            "{body:?}"
        );

        let (status, body) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/recurring-rules",
                serde_json::json!({
                    "locationId": loc.to_string(), "durationMinutes": 30,
                    "runTimesLocal": ["09:00"], "daysOfWeek": [1], "timezone": "UTC",
                    "shortlinkMode": "none",
                }),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "rule create {loc}: {body:?}"
        );
    }

    // A rule that already uses a since-deactivated location stays editable.
    sqlx::query("UPDATE locations SET is_active = false WHERE id = $1")
        .bind(location_id)
        .execute(&db)
        .await
        .unwrap();
    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/recurring-rules/{rule_id}"),
            serde_json::json!({ "description": "still editable" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

/// F3: pausing a rule frees its link for other uses but the rule remembers it,
/// so resuming re-locks it; if another active rule took it the resume is a 409.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn resuming_a_paused_rule_keeps_its_short_link() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI008").await;
    let link = code();
    let rule_id = create_rule(
        &client,
        &app,
        location_id,
        serde_json::json!({ "shortlinkMode": "custom", "customShortCode": link }),
    )
    .await;

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/pause"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["isActive"], false);
    assert_eq!(body["lockedShortCode"], serde_json::json!(link));

    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/resume"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["isActive"], true);
    assert_eq!(body["lockedShortCode"], serde_json::json!(link));

    // While paused, another rule may take the (released) link ...
    client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/pause"),
            serde_json::json!({}),
        )
        .await;
    let other = create_rule(
        &client,
        &app,
        location_id,
        serde_json::json!({
            "shortlinkMode": "existing", "existingShortCode": link, "forceReassign": true,
        }),
    )
    .await;
    // ... and then this one can't silently resume without it.
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/resume"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(
        body["message"].as_str().unwrap().contains(&link),
        "{body:?}"
    );
    let active: bool =
        sqlx::query_scalar("SELECT is_active FROM recurring_session_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(!active);

    // Once the other rule is paused the link is free again.
    client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{other}/pause"),
            serde_json::json!({}),
        )
        .await;
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/resume"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

/// F3 x batch archive: a rule paused because its batch was deleted keeps its
/// link too, so fixing the batch and resuming doesn't leave it link-less.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn batch_archive_pause_keeps_the_rules_short_link() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, "DI009").await;
    let link = code();
    let rule_id = create_rule(
        &client,
        &app,
        location_id,
        serde_json::json!({
            "batchId": batch_id.to_string(), "shortlinkMode": "custom", "customShortCode": link,
        }),
    )
    .await;
    let (status, body) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/batches/{batch_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["rulesPaused"], 1);
    let (active, locked): (bool, Option<String>) = sqlx::query_as(
        "SELECT is_active, locked_short_code FROM recurring_session_rules WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!active);
    assert_eq!(locked.as_deref(), Some(link.as_str()));
}

/// F13: a short link locked to a recurring rule can't be deleted (FK) — 409,
/// not 500; `?page=0` is clamped; the list reports a total.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn rule_locked_short_link_delete_is_409_and_listing_paginates() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI010").await;
    let link = code();
    let rule_id = create_rule(
        &client,
        &app,
        location_id,
        serde_json::json!({ "shortlinkMode": "custom", "customShortCode": link }),
    )
    .await;

    let (status, body) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/shortlinks/{link}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(
        body["message"].as_str().unwrap().contains("recurring rule"),
        "{body:?}"
    );
    // Still locked when paused.
    client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/pause"),
            serde_json::json!({}),
        )
        .await;
    let (status, _) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/shortlinks/{link}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Pagination: 3 extra unattached links, page through 2 at a time.
    for _ in 0..3 {
        let (status, _) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/shortlinks",
                serde_json::json!({ "shortCode": "" }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = client
        .get(&app, "/api/admin/shortlinks?page=0&limit=2")
        .await;
    assert_eq!(status, StatusCode::OK, "page=0 must be clamped: {body:?}");
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["shortLinks"].as_array().unwrap().len(), 2);
    let total = body["total"].as_i64().unwrap();
    assert!(total >= 4, "{body:?}");
    let (_, p2) = client
        .get(&app, "/api/admin/shortlinks?page=2&limit=2")
        .await;
    assert_ne!(p2["shortLinks"][0]["_id"], body["shortLinks"][0]["_id"]);
    let (status, _) = client.get(&app, "/api/admin/shortlinks?limit=-5").await;
    assert_eq!(status, StatusCode::OK);
}

/// F16: a partial `PUT /locations/{id}` is validated as the merged record —
/// out-of-range or empty fields are a 400 and leave the row untouched; valid
/// partial updates still work.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn partial_location_updates_are_validated() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, _) = seed_location_and_batch(&db, admin_id, "DI011").await;
    let uri = format!("/api/admin/locations/{location_id}");

    for bad in [
        serde_json::json!({ "latitude": 999 }),
        serde_json::json!({ "longitude": -181 }),
        serde_json::json!({ "radiusMeters": -10 }),
        serde_json::json!({ "radiusMeters": 20000 }),
        serde_json::json!({ "name": "" }),
    ] {
        let (status, body) = client.mutate(&app, "PUT", &uri, bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body:?}");
    }
    let (lat, radius, name): (f64, f64, String) =
        sqlx::query_as("SELECT latitude, radius_meters, name FROM locations WHERE id = $1")
            .bind(location_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((lat, radius, name.as_str()), (12.9716, 100.0, "Test Hall"));

    let (status, body) = client
        .mutate(
            &app,
            "PUT",
            &uri,
            serde_json::json!({ "radiusMeters": 250 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["radiusMeters"], 250.0);
}
