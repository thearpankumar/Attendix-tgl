//! Admin user management identity rules (case-insensitive uniqueness, 409s,
//! password policy, clearable optional fields), manual-attendance edit-window
//! consistency (mark vs undo), strict UUID request inputs, literal `%`/`_`
//! search and clearable descriptions.

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn super_client(app: &axum::Router, db: &sqlx::PgPool, prefix: &str) -> (Client, Uuid) {
    let u = unique(prefix);
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    (Client::login(app, &u, PW).await, id)
}

async fn create_user(
    client: &Client,
    app: &axum::Router,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    client.mutate(app, "POST", "/api/admin/users", body).await
}

async fn create_mentor(client: &Client, app: &axum::Router, prefix: &str) -> (String, String) {
    let username = unique(prefix);
    let (status, body) = create_user(
        client,
        app,
        serde_json::json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": "mentor-password-123",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    (body["_id"].as_str().unwrap().to_string(), username)
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn username_and_email_are_case_insensitively_unique_with_409s() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db, "idn-super").await;

    let name = unique("Idn-User");
    let (status, body) = create_user(
        &client,
        &app,
        serde_json::json!({
            "username": name,
            "email": format!("  {}@Example.COM ", name),
            "password": "password-123",
            "fullName": "  Some One ",
            "collegeName": "   ",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    // Email is trimmed + lower-cased; blank optional text is stored as null.
    assert_eq!(
        body["email"].as_str().unwrap(),
        format!("{}@example.com", name.to_lowercase())
    );
    assert_eq!(body["fullName"], "Some One");
    assert!(body["collegeName"].is_null());
    let first_id = body["_id"].as_str().unwrap().to_string();

    // Same username, different case -> 409.
    let (status, body) = create_user(
        &client,
        &app,
        serde_json::json!({
            "username": name.to_uppercase(),
            "email": format!("other-{}@example.com", name.to_lowercase()),
            "password": "password-123",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert_eq!(body["message"], "Username already exists");

    // Same email, different case -> 409.
    let (status, body) = create_user(
        &client,
        &app,
        serde_json::json!({
            "username": unique("idn-other"),
            "email": format!("{}@EXAMPLE.com", name.to_uppercase()),
            "password": "password-123",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert_eq!(body["message"], "Email already exists");

    // Update to another account's email (exact and case-variant) -> 409, not 500.
    let (second_id, _) = create_mentor(&client, &app, "idn-second").await;
    for email in [
        format!("{}@example.com", name.to_lowercase()),
        format!("{}@EXAMPLE.COM", name.to_uppercase()),
    ] {
        let (status, body) = client
            .mutate(
                &app,
                "PATCH",
                &format!("/api/admin/users/{second_id}"),
                serde_json::json!({ "email": email }),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{email}: {body:?}");
        assert_eq!(body["message"], "Email already exists");
    }

    // Re-saving an account's own email (even in another case) is fine.
    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{first_id}"),
            serde_json::json!({ "email": format!("{}@EXAMPLE.com", name.to_uppercase()) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    // The database itself also refuses a case-variant (the safety net).
    let dup = sqlx::query(
        "INSERT INTO admins (id, username, email, password, role) VALUES ($1, $2, $3, 'x', 'admin')",
    )
    .bind(Uuid::new_v4())
    .bind(name.to_uppercase())
    .bind(format!("db-{}@example.com", name.to_lowercase()))
    .execute(&db)
    .await;
    assert!(dup.is_err(), "lower(username) unique index must reject it");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn password_policy_is_eight_characters_everywhere() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db, "pwd-super").await;

    let u = unique("pwd-user");
    for short in ["123456", "1234567"] {
        let (status, body) = create_user(
            &client,
            &app,
            serde_json::json!({ "username": u, "email": format!("{u}@example.com"), "password": short }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{short}: {body:?}");
        assert_eq!(body["message"], "Password must be at least 8 characters");
    }
    let (status, body) = create_user(
        &client,
        &app,
        serde_json::json!({ "username": u, "email": format!("{u}@example.com"), "password": "12345678" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let id = body["_id"].as_str().unwrap().to_string();

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{id}"),
            serde_json::json!({ "newPassword": "1234567" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/users/{id}"),
            serde_json::json!({ "newPassword": "new-password-1" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Self-service change.
    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            "/api/admin/profile/password",
            serde_json::json!({ "currentPassword": PW, "newPassword": "short12" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");

    // A legacy account whose (6-char) password predates the policy can still log in.
    let legacy = unique("pwd-legacy");
    seed_admin(
        &db,
        &legacy,
        &format!("{legacy}@example.com"),
        "abc123",
        "admin",
    )
    .await;
    let _ = Client::login(&app, &legacy, "abc123").await;
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn user_optional_fields_can_be_cleared_and_omitted_fields_are_kept() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db, "clr-super").await;

    let u = unique("clr-user");
    let (status, body) = create_user(
        &client,
        &app,
        serde_json::json!({
            "username": u, "email": format!("{u}@example.com"), "password": "password-123",
            "fullName": "Full Name", "collegeName": "College",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let id = body["_id"].as_str().unwrap().to_string();
    let url = format!("/api/admin/users/{id}");

    let (_, body) = client
        .mutate(&app, "PATCH", &url, serde_json::json!({ "isActive": true }))
        .await;
    assert_eq!(body["fullName"], "Full Name", "omitted keeps");
    assert_eq!(body["collegeName"], "College");

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &url,
            serde_json::json!({ "fullName": null, "collegeName": "  " }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["fullName"].is_null(), "null clears: {body:?}");
    assert!(body["collegeName"].is_null(), "blank clears: {body:?}");

    let (_, body) = client
        .mutate(
            &app,
            "PATCH",
            &url,
            serde_json::json!({ "fullName": " New " }),
        )
        .await;
    assert_eq!(body["fullName"], "New");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn descriptions_on_locations_and_rules_can_be_cleared() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db, "dsc-super").await;

    let (status, loc) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/locations",
            serde_json::json!({
                "name": unique("dsc-loc"), "latitude": 12.97, "longitude": 77.59,
                "radiusMeters": 100, "description": "first",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loc:?}");
    let lid = loc["_id"].as_str().unwrap().to_string();
    let url = format!("/api/admin/locations/{lid}");

    let (_, body) = client
        .mutate(&app, "PUT", &url, serde_json::json!({ "isActive": true }))
        .await;
    assert_eq!(body["description"], "first", "omitted keeps");
    let (_, body) = client
        .mutate(&app, "PUT", &url, serde_json::json!({ "description": "" }))
        .await;
    assert!(body["description"].is_null(), "empty clears: {body:?}");
    let (_, body) = client
        .mutate(
            &app,
            "PUT",
            &url,
            serde_json::json!({ "description": "second" }),
        )
        .await;
    assert_eq!(body["description"], "second");
    let (_, body) = client
        .mutate(
            &app,
            "PUT",
            &url,
            serde_json::json!({ "description": null }),
        )
        .await;
    assert!(body["description"].is_null(), "null clears: {body:?}");

    let (status, rule) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/recurring-rules",
            serde_json::json!({
                "locationId": lid, "durationMinutes": 30, "runTimesLocal": ["09:00"],
                "daysOfWeek": [0, 1, 2, 3, 4, 5, 6], "timezone": "UTC",
                "shortlinkMode": "none", "description": "rule text",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rule:?}");
    let rurl = format!(
        "/api/admin/recurring-rules/{}",
        rule["_id"].as_str().unwrap()
    );
    let (_, body) = client
        .mutate(
            &app,
            "PATCH",
            &rurl,
            serde_json::json!({ "durationMinutes": 45 }),
        )
        .await;
    assert_eq!(body["description"], "rule text", "omitted keeps: {body:?}");
    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &rurl,
            serde_json::json!({ "description": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["description"].is_null(), "empty clears: {body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn malformed_uuid_inputs_are_400_not_silently_dropped() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db, "uid-super").await;

    for uri in [
        "/api/admin/sessions?locationId=not-a-uuid",
        "/api/admin/sessions?batchIds=not-a-uuid",
        "/api/admin/sessions/stats-overview?locationId=nope",
        "/api/admin/shortlinks?session_id=nope",
    ] {
        let (status, body) = client.get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body:?}");
    }
    for (uri, body) in [
        (
            "/api/admin/sessions/export-bulk",
            serde_json::json!({ "sessionIds": ["nope"] }),
        ),
        (
            "/api/admin/sessions/export-filtered",
            serde_json::json!({ "batchIds": ["nope"] }),
        ),
        (
            "/api/admin/students/lookup/export",
            serde_json::json!({ "studentIds": ["nope"] }),
        ),
        (
            "/api/admin/shortlinks",
            serde_json::json!({ "sessionId": "nope" }),
        ),
    ] {
        let (status, resp) = client.mutate(&app, "POST", uri, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {resp:?}");
    }
}

/// `%` and `_` typed in a search box match only themselves.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn search_treats_percent_and_underscore_literally() {
    let (app, db) = create_test_app().await;
    let (client, super_id) = super_client(&app, &db, "lik-super").await;

    let tag = unique("lk").replace('-', "");
    let mut batch_ids = Vec::new();
    for (name, student) in [
        (format!("{tag}_a%b"), format!("{tag}_n1")),
        (format!("{tag}Xa-b"), format!("{tag}Xn1")),
    ] {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO batches (id, name, created_by) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(&name)
        .bind(super_id)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO students (id, batch_id, position, name, roll_number) VALUES ($1, $2, 0, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(&student)
        .bind(student.to_uppercase())
        .execute(&db)
        .await
        .unwrap();
        batch_ids.push(id);
    }

    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|b| b["name"].as_str().unwrap().to_string())
            .collect()
    };

    // Batches list: `_` and `%` are literal.
    let (status, body) = client
        .get(&app, &format!("/api/admin/batches?search={tag}_a"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec![format!("{tag}_a%b")], "{body:?}");
    let (_, body) = client
        .get(&app, &format!("/api/admin/batches?search={tag}_a%25b"))
        .await;
    assert_eq!(names(&body), vec![format!("{tag}_a%b")], "{body:?}");
    let (_, body) = client.get(&app, "/api/admin/batches?search=%25").await;
    assert!(
        names(&body).iter().all(|n| n.contains('%')),
        "a bare % must match only names containing a literal %: {body:?}"
    );
    // Without escaping the wildcard form would also have matched the other batch.
    let (_, body) = client
        .get(&app, &format!("/api/admin/batches?search={tag}%25"))
        .await;
    assert!(names(&body).iter().all(|n| n.contains('%')), "{body:?}");

    // Per-batch student search.
    let (status, body) = client
        .get(
            &app,
            &format!("/api/admin/batches/{}/students?search=_n1", batch_ids[0]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["students"].as_array().unwrap().len(), 1, "{body:?}");
    let (_, body) = client
        .get(
            &app,
            &format!("/api/admin/batches/{}/students?search=%25", batch_ids[1]),
        )
        .await;
    assert_eq!(body["students"].as_array().unwrap().len(), 0, "{body:?}");

    // Cross-batch student lookup.
    let (_, body) = client
        .get(&app, &format!("/api/admin/students?search={tag}_n1"))
        .await;
    assert_eq!(body["students"].as_array().unwrap().len(), 1, "{body:?}");
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn undo_follows_the_same_exam_window_and_active_rules_as_marking() {
    let (app, db) = create_test_app().await;
    let (client, super_id) = super_client(&app, &db, "und-super").await;
    let (mentor_id, mentor_username) = create_mentor(&client, &app, "und-mentor").await;
    let mentor = Client::login(&app, &mentor_username, "mentor-password-123").await;

    let (location_id, batch_id) = seed_location_and_batch(&db, super_id, "UND001").await;
    sqlx::query(
        "INSERT INTO students (id, batch_id, position, name, roll_number) VALUES ($1, $2, 1, 'Second', 'UND002')",
    )
    .bind(Uuid::new_v4())
    .bind(batch_id)
    .execute(&db)
    .await
    .unwrap();

    let (status, session) = client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "collegeName": "XYZ College",
                "startsAt": (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339(),
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session:?}");
    let sid = session["_id"].as_str().unwrap().to_string();
    let mark = format!("/api/admin/sessions/{sid}/attendance/manual");
    let undo = |roll: &str| format!("/api/admin/sessions/{sid}/attendance/manual/{roll}");
    let present = |roll: &str| serde_json::json!({ "rollNumber": roll, "status": "present" });

    // Open exam: mark + undo work.
    let (status, body) = mentor.mutate(&app, "POST", &mark, present("UND001")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = mentor.mutate(&app, "POST", &mark, present("UND002")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = mentor
        .mutate(&app, "DELETE", &undo("UND002"), serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    // Exam closed: both blocked, and the existing mark survives the rejected undo.
    sqlx::query("UPDATE sessions SET expires_at = now() - interval '2 minutes' WHERE id = $1")
        .bind(Uuid::parse_str(&sid).unwrap())
        .execute(&db)
        .await
        .unwrap();
    let (status, body) = mentor.mutate(&app, "POST", &mark, present("UND002")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let (status, body) = mentor
        .mutate(&app, "DELETE", &undo("UND001"), serde_json::json!({}))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "undo on closed exam: {body:?}"
    );
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = $1")
            .bind(Uuid::parse_str(&sid).unwrap())
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(remaining, 1, "rejected undo must not delete the mark");

    // Deactivated: both blocked (reopen the window first so only is_active differs).
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '20 minutes' WHERE id = $1")
        .bind(Uuid::parse_str(&sid).unwrap())
        .execute(&db)
        .await
        .unwrap();
    let (status, _) = client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{sid}/deactivate"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = mentor.mutate(&app, "POST", &mark, present("UND002")).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "mark on deactivated: {body:?}"
    );
    let (status, body) = mentor
        .mutate(&app, "DELETE", &undo("UND001"), serde_json::json!({}))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "undo on deactivated: {body:?}"
    );
    // Super-admins are held to the same rules as mentors.
    let (status, _) = client
        .mutate(&app, "DELETE", &undo("UND001"), serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
