//! Roll numbers are stored trimmed and upper-cased — on batch import, manual
//! add/edit and roster sync — matching attendance rows, exports and the
//! mentor roster, while the roll-change cascade (attendance rows follow a
//! corrected roll number) keeps working.

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn insert_session(db: &sqlx::PgPool, admin_id: Uuid, batch_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO sessions (id, token_hash, token_prefix, created_by, batch_id, is_active, expires_at, created_at) \
         VALUES ($1, $2, 'rc', $3, $4, true, now() + interval '1 hour', now()) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4().to_string())
    .bind(admin_id)
    .bind(batch_id)
    .fetch_one(db)
    .await
    .unwrap()
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn roll_numbers_are_uppercased_on_add_edit_and_sync_and_the_cascade_still_works() {
    let (app, db) = create_test_app().await;
    let u = unique("rcase-super");
    let admin_id = seed_admin(&db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    let client = Client::login(&app, &u, PW).await;

    let batch_id: Uuid = sqlx::query_scalar(
        "INSERT INTO batches (id, name, created_by) VALUES ($1, 'Case Batch', $2) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    let base = format!("/api/admin/batches/{batch_id}/students");

    // Manual add.
    let (status, added) = client
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": "Alice", "rollNumber": "  zzr001 " }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{added:?}");
    assert_eq!(added["rollNumber"], "ZZR001");
    let alice_id = added["id"].as_str().unwrap().to_string();

    // A case-insensitive duplicate is still refused.
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &base,
            serde_json::json!({ "name": "Dup", "rollNumber": "zzr001" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Roster sync (merge): new rows are stored upper-case, matches are case-insensitive.
    let (status, body) = client
        .mutate(
            &app,
            "PUT",
            &base,
            serde_json::json!({
                "mode": "merge",
                "students": [
                    { "name": "Alice", "rollNumber": "ZZr001" },
                    { "name": "Bob", "rollNumber": "zzr002" },
                ],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let rolls: Vec<String> = sqlx::query_scalar(
        "SELECT roll_number FROM students WHERE batch_id = $1 ORDER BY position",
    )
    .bind(batch_id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(rolls, vec!["ZZR001", "ZZR002"]);

    // Edit with a lower-case correction: stored upper-case, attendance follows.
    let session_id = insert_session(&db, admin_id, batch_id).await;
    sqlx::query(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, \
            source, status, captured_at \
         ) VALUES ($1, $2, 'Alice', 'ZZR001', '', '', 0, 0, 0, true, 'self_submitted', 'present', now())",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .execute(&db)
    .await
    .unwrap();

    let (status, edited) = client
        .mutate(
            &app,
            "PATCH",
            &format!("{base}/{alice_id}"),
            serde_json::json!({ "name": "Alice", "rollNumber": "zzr009" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited:?}");
    assert_eq!(edited["rollNumber"], "ZZR009");
    let moved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND roll_number = 'ZZR009'",
    )
    .bind(session_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        moved, 1,
        "attendance must follow the roll-number correction"
    );

    // A case-only edit of a legacy lower-case row just normalises it.
    let legacy_id: Uuid = sqlx::query_scalar(
        "INSERT INTO students (id, batch_id, position, name, roll_number) VALUES ($1, $2, 9, 'Legacy', 'zzlegacy') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(batch_id)
    .fetch_one(&db)
    .await
    .unwrap();
    let (status, edited) = client
        .mutate(
            &app,
            "PATCH",
            &format!("{base}/{legacy_id}"),
            serde_json::json!({ "name": "Legacy", "rollNumber": "zzlegacy" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited:?}");
    assert_eq!(edited["rollNumber"], "ZZLEGACY");
}

/// The mentor roster shows upper-case even for a row that predates the rule.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn mentor_roster_displays_uppercase_roll_numbers() {
    let (app, db) = create_test_app().await;
    let u = unique("rdisp-super");
    let admin_id = seed_admin(&db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    let client = Client::login(&app, &u, PW).await;
    let (_, batch_id) = seed_location_and_batch(&db, admin_id, "zzdisp1").await;
    let session_id = insert_session(&db, admin_id, batch_id).await;

    let (status, roster) = client
        .get(&app, &format!("/api/admin/sessions/{session_id}/roster"))
        .await;
    assert_eq!(status, StatusCode::OK, "{roster:?}");
    assert_eq!(roster["students"][0]["rollNumber"], "ZZDISP1");
}
