//! Batch deletion (archive-vs-delete) and roster editing (upload merge /
//! replace, spreadsheet-style row edits).

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn super_client(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let u = unique("batch-super");
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    (Client::login(app, &u, PW).await, id)
}

async fn plain_admin_client(app: &axum::Router, db: &sqlx::PgPool) -> Client {
    let u = unique("batch-admin");
    seed_admin(db, &u, &format!("{u}@example.com"), PW, "admin").await;
    Client::login(app, &u, PW).await
}

async fn count(db: &sqlx::PgPool, sql: &str, id: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn deleting_a_batch_with_sessions_archives_it_and_keeps_history_working() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let roll = format!("DEL{}", &Uuid::new_v4().simple().to_string()[..6]);
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, &roll).await;

    let (status, session) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session:?}");
    let session_id = session["_id"].as_str().unwrap().to_string();

    // An attendance row (keyed by roll number text) that must survive.
    sqlx::query(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, \
            source, status, captured_at \
         ) VALUES ($1, $2, 'Test Student', $3, 'https://example.com/p.jpg', 'photos/p', 12.97, 77.59, 0.0, true, \
            'self_submitted', 'present', now())",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::parse_str(&session_id).unwrap())
    .bind(&roll)
    .execute(&db)
    .await
    .unwrap();

    // An active recurring rule on the same batch.
    let (status, rule) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/recurring-rules",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "durationMinutes": 30,
                "runTimesLocal": ["09:00"],
                "daysOfWeek": [1, 2, 3, 4, 5],
                "timezone": "UTC",
                "shortlinkMode": "none",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rule:?}");
    let rule_id = rule["_id"].as_str().unwrap().to_string();

    // The confirmation dialog's data.
    let (status, impact) = client
        .get(
            &app,
            &format!("/api/admin/batches/{batch_id}/delete-impact"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{impact:?}");
    assert_eq!(impact["mode"], "archived");
    assert_eq!(impact["sessionCount"], 1);
    assert_eq!(impact["activeRuleCount"], 1);

    // Used to be an opaque 500 (sessions_batch_id_fkey).
    let (status, body) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/batches/{batch_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["mode"], "archived");
    assert_eq!(body["sessionsKept"], 1);
    assert_eq!(body["rulesPaused"], 1);

    // Gone from every admin-facing batch surface...
    let (_, list) = client.get(&app, "/api/admin/batches").await;
    assert!(
        !list
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["_id"] == batch_id.to_string()),
        "deleted batch must not be listed"
    );
    for uri in [
        format!("/api/admin/batches/{batch_id}"),
        format!("/api/admin/batches/{batch_id}/overview"),
        format!("/api/admin/batches/{batch_id}/students"),
    ] {
        let (status, _) = client.get(&app, &uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    // ...and can't be deleted twice.
    let (status, _) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/batches/{batch_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // ...but the historical session still reads end to end.
    let (status, s) = client
        .get(&app, &format!("/api/admin/sessions/{session_id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["batchName"], "Test Batch");
    for suffix in ["stats", "absent", "roster", "attendance"] {
        let (status, body) = client
            .get(&app, &format!("/api/admin/sessions/{session_id}/{suffix}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{suffix}: {body:?}");
    }
    let (_, roster) = client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert!(
        roster.to_string().contains(&roll.to_uppercase()),
        "roster keeps the students: {roster:?}"
    );
    let (status, _) = client
        .get(&app, &format!("/api/admin/sessions/{session_id}/export"))
        .await;
    assert_eq!(status, StatusCode::OK);

    // Roster + attendance rows are physically kept.
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM students WHERE batch_id = $1",
            batch_id
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM attendances WHERE session_id = $1",
            Uuid::parse_str(&session_id).unwrap()
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM batches WHERE id = $1 AND deleted_at IS NOT NULL",
            batch_id
        )
        .await,
        1
    );

    // The rule was paused with an explanation and can't be resumed or
    // re-pointed at the deleted batch; new sessions can't use it either.
    let (_, rules) = client.get(&app, "/api/admin/recurring-rules").await;
    let r = rules
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["_id"] == rule_id)
        .expect("rule still listed");
    assert_eq!(r["isActive"], false);
    assert!(
        r["lastError"].as_str().unwrap().contains("was deleted"),
        "{r:?}"
    );
    let (status, body) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/recurring-rules/{rule_id}/resume"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("batch was deleted"),
        "{body:?}"
    );
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Audit trail.
    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_log WHERE event = 'batch_deleted' AND admin_id = $1",
    )
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audited, 1);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn deleting_an_unreferenced_batch_removes_it_and_other_admins_cannot() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (_, batch_id) = seed_location_and_batch(&db, admin_id, "UNREF1").await;

    let (status, impact) = client
        .get(
            &app,
            &format!("/api/admin/batches/{batch_id}/delete-impact"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(impact["mode"], "deleted");

    // A different, non-super admin does not own it.
    let other = plain_admin_client(&app, &db).await;
    let (status, _) = other
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/batches/{batch_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = client
        .mutate(
            &app,
            "DELETE",
            &format!("/api/admin/batches/{batch_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["mode"], "deleted");
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM batches WHERE id = $1", batch_id).await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM students WHERE batch_id = $1",
            batch_id
        )
        .await,
        0
    );
}

fn roster_of(body: &serde_json::Value) -> Vec<(String, String)> {
    body["students"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["rollNumber"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn upload_merge_and_replace_preview_then_apply() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (_, batch_id) = seed_location_and_batch(&db, admin_id, "r001").await;
    sqlx::query("INSERT INTO students (id, batch_id, position, name, roll_number) VALUES ($1, $2, 1, 'Bob', 'R002')")
        .bind(Uuid::new_v4())
        .bind(batch_id)
        .execute(&db)
        .await
        .unwrap();
    let base = format!("/api/admin/batches/{batch_id}/students");

    // File: header detected via aliases; R001 differs only by case; Bob
    // renamed; Cara new; one duplicate row; one row missing a roll number.
    let csv = b"Reg. No,Student Name,Email\nR001,Test Student,\nr002,Robert,r@x.test\nR003,Cara,c@x.test\nr003,Cara Dup,d@x.test\n,Nameless,\n";
    let (status, parsed) = client
        .post_file(&app, &format!("{base}/parse"), "roster.csv", csv)
        .await;
    assert_eq!(status, StatusCode::OK, "{parsed:?}");
    assert_eq!(parsed["students"].as_array().unwrap().len(), 3);
    assert!(
        parsed["errors"].as_array().unwrap().len() >= 2,
        "{parsed:?}"
    );
    let students = parsed["students"].clone();

    // Merge preview: nothing is written.
    let (status, diff) = client
        .mutate(
            &app,
            "POST",
            &format!("{base}/preview"),
            serde_json::json!({ "mode": "merge", "students": students }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{diff:?}");
    assert_eq!(diff["added"].as_array().unwrap().len(), 1);
    assert_eq!(diff["updated"].as_array().unwrap().len(), 1);
    assert_eq!(diff["removed"].as_array().unwrap().len(), 0);
    assert_eq!(diff["unchanged"], 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM students WHERE batch_id = $1",
            batch_id
        )
        .await,
        2
    );

    // Apply merge.
    let (status, res) = client
        .mutate(
            &app,
            "PUT",
            &base,
            serde_json::json!({ "mode": "merge", "students": students, "source": "excel" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{res:?}");
    assert_eq!(res["studentCount"], 3);
    let (_, detail) = client
        .get(&app, &format!("/api/admin/batches/{batch_id}"))
        .await;
    assert_eq!(
        roster_of(&detail),
        vec![
            ("r001".to_string(), "Test Student".to_string()),
            ("R002".to_string(), "Robert".to_string()),
            ("R003".to_string(), "Cara".to_string()),
        ]
    );

    // Replace with a roster that drops everyone but Cara: Robert and Test
    // Student are removed (their attendance, if any, is untouched).
    let attendance_session: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (id, location_id, batch_id, token_hash, token_prefix, created_by, is_active, expires_at) \
         SELECT $1, NULL, $2, 'h', 'p', $3, false, now() RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(batch_id)
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attendances (id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, source, status, captured_at) \
         VALUES ($1, $2, 'Robert', 'R002', 'u', 'p', 0, 0, 0, true, 'self_submitted', 'present', now())",
    )
    .bind(Uuid::new_v4())
    .bind(attendance_session)
    .execute(&db)
    .await
    .unwrap();

    let replace = serde_json::json!({ "mode": "replace", "students": [
        { "name": "Cara", "rollNumber": "R003" },
        { "name": "Dan", "rollNumber": "R004", "email": "dan@x.test" },
    ]});
    let (_, diff) = client
        .mutate(&app, "POST", &format!("{base}/preview"), replace.clone())
        .await;
    assert_eq!(diff["removed"].as_array().unwrap().len(), 2);
    assert_eq!(diff["added"].as_array().unwrap().len(), 1);
    assert_eq!(diff["unchanged"], 1);
    let (status, _) = client.mutate(&app, "PUT", &base, replace).await;
    assert_eq!(status, StatusCode::OK);
    let (_, detail) = client
        .get(&app, &format!("/api/admin/batches/{batch_id}"))
        .await;
    assert_eq!(
        roster_of(&detail),
        vec![
            ("R003".to_string(), "Cara".to_string()),
            ("R004".to_string(), "Dan".to_string())
        ]
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM attendances WHERE session_id = $1",
            attendance_session
        )
        .await,
        1,
        "removing a student must not touch past attendance"
    );

    // Validation: duplicates (case-insensitive) and empty replace are rejected.
    let (status, body) = client
        .mutate(
            &app,
            "PUT",
            &base,
            serde_json::json!({ "mode": "merge", "students": [
            { "name": "A", "rollNumber": "x1" }, { "name": "B", "rollNumber": "X1" } ]}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let (status, _) = client
        .mutate(
            &app,
            "PUT",
            &base,
            serde_json::json!({ "mode": "replace", "students": [] }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Role scoping + audit.
    let other = plain_admin_client(&app, &db).await;
    let (status, _) = other
        .mutate(&app, "PUT", &base, serde_json::json!({ "mode": "merge", "students": [{ "name": "Z", "rollNumber": "Z1" }]}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other
        .post_file(&app, &format!("{base}/parse"), "r.csv", csv)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_log WHERE event = 'batch_roster_synced' AND admin_id = $1",
    )
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audited, 2);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn manual_row_add_edit_delete_with_duplicate_checks() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (_, batch_id) = seed_location_and_batch(&db, admin_id, "M001").await;
    let base = format!("/api/admin/batches/{batch_id}/students");

    let (status, added) = client
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": " Eve ", "rollNumber": " m002 ", "email": "eve@x.test" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{added:?}");
    // Roll numbers are stored trimmed + upper-cased.
    assert_eq!(added["rollNumber"], "M002");
    assert_eq!(added["position"], 1);
    let eve_id = added["id"].as_str().unwrap().to_string();

    // Case-insensitive duplicate of an existing roll number.
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": "Dup", "rollNumber": "M001" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": "Bad", "rollNumber": "M9", "email": "nope" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Edit: rename, clear email, change roll; colliding roll is a conflict,
    // keeping your own roll (even re-cased) is fine.
    let (status, edited) = client
        .mutate(
            &app,
            "PATCH",
            &format!("{base}/{eve_id}"),
            serde_json::json!({ "name": "Eve E", "rollNumber": "M002", "email": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited:?}");
    assert_eq!(edited["name"], "Eve E");
    assert!(edited["email"].is_null());
    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("{base}/{eve_id}"),
            serde_json::json!({ "name": "Eve E", "rollNumber": "m001" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Past attendance keeps the old roll number (not rewritten).
    // Delete.
    let (status, _) = client
        .mutate(
            &app,
            "DELETE",
            &format!("{base}/{eve_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = client
        .mutate(
            &app,
            "DELETE",
            &format!("{base}/{eve_id}"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let other = plain_admin_client(&app, &db).await;
    let (status, _) = other
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": "X", "rollNumber": "X1" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let events: Vec<String> = sqlx::query_scalar(
        "SELECT event FROM admin_audit_log WHERE admin_id = $1 AND event LIKE 'batch_student_%' ORDER BY seq",
    )
    .bind(admin_id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        events,
        [
            "batch_student_added",
            "batch_student_updated",
            "batch_student_removed"
        ]
    );
}
